//! Plain data types produced by the collector and consumed by the UI.
//!
//! Nothing in here depends on `sysinfo`, so the UI stays easy to test.

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub sys: SysInfo,
    pub cpu_brand: String,
    /// Global CPU usage, 0..=100.
    pub cpu_total: f32,
    /// Per-core CPU usage, 0..=100 each.
    pub cpu_cores: Vec<f32>,
    pub mem: MemStats,
    pub nets: Vec<NetStats>,
    pub disks: Vec<DiskStats>,
    pub procs: Vec<ProcInfo>,
}

#[derive(Clone, Debug, Default)]
pub struct SysInfo {
    pub hostname: String,
    pub os: String,
    pub uptime_secs: u64,
    pub load: [f64; 3],
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MemStats {
    pub total: u64,
    pub used: u64,
    pub swap_total: u64,
    pub swap_used: u64,
}

impl MemStats {
    pub fn used_ratio(&self) -> f64 {
        ratio(self.used, self.total)
    }

    pub fn swap_ratio(&self) -> f64 {
        ratio(self.swap_used, self.swap_total)
    }
}

#[derive(Clone, Debug, Default)]
pub struct NetStats {
    pub name: String,
    /// Bytes per second received since the previous snapshot.
    pub rx_rate: u64,
    /// Bytes per second transmitted since the previous snapshot.
    pub tx_rate: u64,
    pub rx_total: u64,
    pub tx_total: u64,
}

#[derive(Clone, Debug, Default)]
pub struct DiskStats {
    pub name: String,
    pub mount: String,
    pub fs: String,
    pub kind: String,
    pub total: u64,
    pub available: u64,
}

impl DiskStats {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.available)
    }

    pub fn used_ratio(&self) -> f64 {
        ratio(self.used(), self.total)
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
    /// CPU usage normalised by core count, so it never exceeds 100.
    pub cpu: f32,
    pub mem: u64,
    pub status: String,
    pub user: String,
}

/// `part / whole` clamped to 0..=1, and 0 when `whole` is 0.
pub fn ratio(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        (part as f64 / whole as f64).clamp(0.0, 1.0)
    }
}

/// Loopback interfaces are excluded from the aggregate network graphs.
pub fn is_loopback(name: &str) -> bool {
    name == "lo" || name == "lo0" || name.to_ascii_lowercase().starts_with("loopback")
}

#[cfg(test)]
pub mod sample {
    use super::*;

    /// A fixed snapshot for unit and render tests.
    pub fn snapshot() -> Snapshot {
        let procs = [
            (1, "init", 0.1, 4 << 20, "Sleep", "root"),
            (42, "firefox", 35.5, 900 << 20, "Run", "alice"),
            (77, "rust-analyzer", 12.0, 600 << 20, "Sleep", "alice"),
            (100, "Code", 5.5, 300 << 20, "Sleep", "alice"),
            (200, "zsh", 0.0, 8 << 20, "Sleep", "alice"),
        ]
        .into_iter()
        .map(|(pid, name, cpu, mem, status, user)| ProcInfo {
            pid,
            name: name.into(),
            cpu,
            mem,
            status: status.into(),
            user: user.into(),
        })
        .collect();

        Snapshot {
            sys: SysInfo {
                hostname: "testbox".into(),
                os: "TestOS 1.0".into(),
                uptime_secs: 90_061,
                load: [0.5, 0.4, 0.3],
            },
            cpu_brand: "Test CPU".into(),
            cpu_total: 42.0,
            cpu_cores: vec![10.0, 50.0, 90.0, 20.0],
            mem: MemStats {
                total: 16 << 30,
                used: 6 << 30,
                swap_total: 4 << 30,
                swap_used: 1 << 30,
            },
            nets: vec![
                NetStats {
                    name: "eth0".into(),
                    rx_rate: 2 << 20,
                    tx_rate: 512 << 10,
                    rx_total: 10 << 30,
                    tx_total: 2 << 30,
                },
                NetStats {
                    name: "lo".into(),
                    rx_rate: 100 << 20,
                    tx_rate: 100 << 20,
                    rx_total: 1 << 30,
                    tx_total: 1 << 30,
                },
            ],
            disks: vec![DiskStats {
                name: "nvme0n1p2".into(),
                mount: "/".into(),
                fs: "ext4".into(),
                kind: "SSD".into(),
                total: 500 << 30,
                available: 200 << 30,
            }],
            procs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_handles_zero_and_overflow() {
        assert_eq!(ratio(5, 0), 0.0);
        assert_eq!(ratio(5, 10), 0.5);
        assert_eq!(ratio(20, 10), 1.0);
    }

    #[test]
    fn loopback_detection() {
        assert!(is_loopback("lo"));
        assert!(is_loopback("lo0"));
        assert!(is_loopback("Loopback Pseudo-Interface 1"));
        assert!(!is_loopback("eth0"));
    }

    #[test]
    fn disk_used_never_underflows() {
        let d = DiskStats {
            total: 10,
            available: 20,
            ..Default::default()
        };
        assert_eq!(d.used(), 0);
    }
}
