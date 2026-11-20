//! Reads the system through `sysinfo` and turns it into [`Snapshot`]s.
//!
//! The collector lives on its own thread so a slow process refresh never
//! freezes the UI.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use sysinfo::{
    Disks, MINIMUM_CPU_UPDATE_INTERVAL, Networks, Pid, ProcessRefreshKind, ProcessesToUpdate,
    System, UpdateKind, Users,
};

use crate::event::{Command, Event};
use crate::model::{DiskStats, MemStats, NetStats, ProcInfo, Snapshot, SysInfo};

pub struct Collector {
    sys: System,
    networks: Networks,
    disks: Disks,
    users: Users,
    last_collect: Instant,
    process_kind: ProcessRefreshKind,
}

impl Collector {
    /// Create the collector and take a first reading.
    ///
    /// CPU and network values are deltas, so they only become meaningful from
    /// the second refresh on. The collector thread waits
    /// [`MINIMUM_CPU_UPDATE_INTERVAL`] before the first real snapshot.
    pub fn new() -> Result<Self, sysinfo::Error> {
        let mut sys = System::new()?;
        // Skip per-thread entries on Linux; we only list processes.
        let process_kind = ProcessRefreshKind::nothing()
            .with_cpu()
            .with_memory()
            .with_user(UpdateKind::OnlyIfNotSet)
            .without_tasks();
        sys.refresh_cpu_all();
        sys.refresh_memory();
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, process_kind);

        Ok(Self {
            sys,
            networks: Networks::new_with_refreshed_list()?,
            disks: Disks::new_with_refreshed_list()?,
            users: Users::new_with_refreshed_list()?,
            last_collect: Instant::now(),
            process_kind,
        })
    }

    pub fn collect(&mut self) -> Snapshot {
        let now = Instant::now();
        let secs = now
            .duration_since(self.last_collect)
            .as_secs_f64()
            .max(0.001);
        self.last_collect = now;

        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.sys
            .refresh_processes_specifics(ProcessesToUpdate::All, true, self.process_kind);
        self.networks.refresh(true);
        self.disks.refresh(true);

        let cores = self.sys.cpus();
        let core_count = cores.len().max(1) as f32;

        let mut nets: Vec<NetStats> = self
            .networks
            .iter()
            .map(|(name, data)| NetStats {
                name: name.to_string_lossy().into_owned(),
                // `received()` is "bytes since the last refresh", so divide
                // by the real elapsed time, not the nominal interval.
                rx_rate: (data.received() as f64 / secs) as u64,
                tx_rate: (data.transmitted() as f64 / secs) as u64,
                rx_total: data.total_received(),
                tx_total: data.total_transmitted(),
            })
            .collect();
        nets.sort_by(|a, b| a.name.cmp(&b.name));

        let disks = self
            .disks
            .iter()
            .filter(|d| d.total_space() > 0)
            .map(|d| DiskStats {
                name: d.name().to_string_lossy().into_owned(),
                mount: d.mount_point().to_string_lossy().into_owned(),
                fs: d.file_system().to_string_lossy().into_owned(),
                kind: d.kind().to_string(),
                total: d.total_space(),
                available: d.available_space(),
            })
            .collect();

        let procs = self
            .sys
            .processes()
            .values()
            .map(|p| ProcInfo {
                pid: p.pid().as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                cpu: p.cpu_usage() / core_count,
                mem: p.memory(),
                status: format!("{:?}", p.status()),
                user: p
                    .user_id()
                    .and_then(|uid| self.users.get_user_by_id(uid))
                    .map(|u| u.name().to_owned())
                    .unwrap_or_default(),
            })
            .collect();

        let load = System::load_average().map(|l| [l.one, l.five, l.fifteen]);

        Snapshot {
            sys: SysInfo {
                hostname: System::host_name().unwrap_or_default(),
                os: System::long_os_version()
                    .or_else(|_| System::name())
                    .unwrap_or_default(),
                uptime_secs: System::uptime().unwrap_or_default(),
                load: load.unwrap_or_default(),
            },
            cpu_brand: cores
                .first()
                .map(|c| c.brand().to_owned())
                .unwrap_or_default(),
            cpu_total: self.sys.global_cpu_usage(),
            cpu_cores: cores.iter().map(|c| c.usage()).collect(),
            mem: MemStats {
                total: self.sys.total_memory(),
                used: self.sys.used_memory(),
                swap_total: self.sys.total_swap(),
                swap_used: self.sys.used_swap(),
            },
            nets,
            disks,
            procs,
        }
    }

    /// Send a kill signal and describe the outcome for the footer.
    pub fn kill(&self, pid: u32) -> String {
        if pid <= 1 || pid == std::process::id() {
            return format!("Refusing to kill PID {pid}");
        }
        match self.sys.process(Pid::from_u32(pid)) {
            Some(p) => {
                let name = p.name().to_string_lossy().into_owned();
                if p.kill() {
                    format!("Sent kill signal to {name} ({pid})")
                } else {
                    format!("Could not kill {name} ({pid}); permission denied?")
                }
            }
            None => format!("Process {pid} no longer exists"),
        }
    }
}

/// Run the collector on its own thread.
///
/// It sends a [`Event::Snapshot`] every `interval_ms` milliseconds (the value
/// is re-read on every loop, so the UI can change it live) and handles
/// [`Command`]s as they arrive. The thread stops when either channel closes.
pub fn spawn(
    mut collector: Collector,
    tx: Sender<Event>,
    commands: Receiver<Command>,
    interval_ms: Arc<AtomicU64>,
) {
    thread::spawn(move || {
        let started = Instant::now();
        let mut last_tick: Option<Instant> = None;
        loop {
            let due = match last_tick {
                None => started + MINIMUM_CPU_UPDATE_INTERVAL,
                Some(t) => t + Duration::from_millis(interval_ms.load(Ordering::Relaxed)),
            };
            let wait = due.saturating_duration_since(Instant::now());
            match commands.recv_timeout(wait) {
                Ok(Command::Kill(pid)) => {
                    if tx.send(Event::Notice(collector.kill(pid))).is_err() {
                        break;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    let snapshot = collector.collect();
                    last_tick = Some(Instant::now());
                    if tx.send(Event::Snapshot(Box::new(snapshot))).is_err() {
                        break;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_real_data() {
        let mut c = Collector::new().expect("sysinfo should initialise");
        thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
        let snap = c.collect();
        assert!(!snap.cpu_cores.is_empty());
        assert!(snap.mem.total > 0);
        assert!(snap.mem.used <= snap.mem.total);
        assert!(!snap.procs.is_empty());
        assert!(snap.cpu_total >= 0.0);
    }

    #[test]
    fn refuses_to_kill_init_and_self() {
        let c = Collector::new().unwrap();
        assert!(c.kill(1).starts_with("Refusing"));
        assert!(c.kill(std::process::id()).starts_with("Refusing"));
    }
}
