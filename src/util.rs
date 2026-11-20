//! Small formatting helpers shared by every tab.

const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];

/// `1536` -> `"1.5 KiB"`.
pub fn fmt_bytes(bytes: u64) -> String {
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// `1536` -> `"1.5 KiB/s"`.
pub fn fmt_rate(bytes_per_sec: u64) -> String {
    format!("{}/s", fmt_bytes(bytes_per_sec))
}

/// `90061` -> `"1d 1h 1m"`.
pub fn fmt_duration(secs: u64) -> String {
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let mins = (secs % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h {mins}m")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else {
        format!("{mins}m {}s", secs % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes() {
        assert_eq!(fmt_bytes(0), "0 B");
        assert_eq!(fmt_bytes(1023), "1023 B");
        assert_eq!(fmt_bytes(1024), "1.0 KiB");
        assert_eq!(fmt_bytes(1536), "1.5 KiB");
        assert_eq!(fmt_bytes(1 << 30), "1.0 GiB");
        assert_eq!(fmt_bytes(5 << 40), "5.0 TiB");
        assert_eq!(fmt_bytes(u64::MAX), "16777216.0 TiB");
    }

    #[test]
    fn rates() {
        assert_eq!(fmt_rate(2048), "2.0 KiB/s");
    }

    #[test]
    fn durations() {
        assert_eq!(fmt_duration(59), "0m 59s");
        assert_eq!(fmt_duration(3_700), "1h 1m");
        assert_eq!(fmt_duration(90_061), "1d 1h 1m");
    }
}
