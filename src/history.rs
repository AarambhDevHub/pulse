//! Fixed-size history buffers that feed the graphs.

use std::collections::VecDeque;

use crate::model::{Snapshot, is_loopback};

/// Number of samples kept for every graph.
pub const HISTORY_LEN: usize = 120;

/// A ring buffer that drops the oldest value when full.
#[derive(Clone, Debug)]
pub struct RingBuf<T> {
    buf: VecDeque<T>,
    cap: usize,
}

impl<T> RingBuf<T> {
    pub fn new(cap: usize) -> Self {
        Self {
            buf: VecDeque::with_capacity(cap),
            cap,
        }
    }

    pub fn push(&mut self, value: T) {
        if self.cap == 0 {
            return;
        }
        if self.buf.len() == self.cap {
            self.buf.pop_front();
        }
        self.buf.push_back(value);
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator {
        self.buf.iter()
    }

    pub fn last(&self) -> Option<&T> {
        self.buf.back()
    }
}

/// Everything the graphs need to draw a trend line.
#[derive(Clone, Debug)]
pub struct History {
    pub cpu_total: RingBuf<f64>,
    /// Aggregate download rate over all non-loopback interfaces, bytes/s.
    pub net_rx: RingBuf<u64>,
    /// Aggregate upload rate over all non-loopback interfaces, bytes/s.
    pub net_tx: RingBuf<u64>,
}

impl Default for History {
    fn default() -> Self {
        Self {
            cpu_total: RingBuf::new(HISTORY_LEN),
            net_rx: RingBuf::new(HISTORY_LEN),
            net_tx: RingBuf::new(HISTORY_LEN),
        }
    }
}

impl History {
    pub fn record(&mut self, snap: &Snapshot) {
        self.cpu_total.push(f64::from(snap.cpu_total));
        let (rx, tx) = snap
            .nets
            .iter()
            .filter(|n| !is_loopback(&n.name))
            .fold((0u64, 0u64), |(rx, tx), n| {
                (rx.saturating_add(n.rx_rate), tx.saturating_add(n.tx_rate))
            });
        self.net_rx.push(rx);
        self.net_tx.push(tx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::sample;

    #[test]
    fn ring_drops_oldest() {
        let mut r = RingBuf::new(3);
        for v in 1..=5 {
            r.push(v);
        }
        assert_eq!(r.iter().copied().collect::<Vec<_>>(), vec![3, 4, 5]);
        assert_eq!(r.len(), 3);
        assert_eq!(r.last(), Some(&5));
    }

    #[test]
    fn zero_capacity_is_a_noop() {
        let mut r = RingBuf::new(0);
        r.push(1);
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn record_skips_loopback() {
        let mut h = History::default();
        h.record(&sample::snapshot());
        assert_eq!(h.net_rx.last(), Some(&(2 << 20)));
        assert_eq!(h.net_tx.last(), Some(&(512 << 10)));
        assert_eq!(h.cpu_total.last(), Some(&42.0));
    }
}
