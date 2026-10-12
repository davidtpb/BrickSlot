//! The live numbers the Home panel shows: how hot the SoC runs, how fast
//! the first core is clocked, and how much memory is in use.

use std::path::Path;

const THERMAL: &str = "/sys/class/thermal/thermal_zone0/temp";
const CPU_FREQ: &str = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq";
const MEMINFO: &str = "/proc/meminfo";

#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct Stats {
    pub temp_c: Option<f32>,
    pub cpu_mhz: Option<u32>,
    pub ram_used_mb: Option<u32>,
}

impl Stats {
    pub fn read() -> Stats {
        Stats::read_from(Path::new("/"))
    }

    /// Reads under `root`, so a test can point it at a fake tree.
    pub fn read_from(root: &Path) -> Stats {
        let read = |p: &str| std::fs::read_to_string(root.join(p.trim_start_matches('/'))).ok();
        Stats {
            temp_c: read(THERMAL).as_deref().and_then(parse_temp),
            cpu_mhz: read(CPU_FREQ).as_deref().and_then(parse_khz),
            ram_used_mb: read(MEMINFO).as_deref().and_then(parse_meminfo),
        }
    }
}

/// The kernel reports millidegrees; some older drivers plain degrees.
pub fn parse_temp(s: &str) -> Option<f32> {
    let v: f32 = s.trim().parse().ok()?;
    Some(if v > 200.0 { v / 1000.0 } else { v })
}

pub fn parse_khz(s: &str) -> Option<u32> {
    let khz: u32 = s.trim().parse().ok()?;
    Some(khz / 1000)
}

/// Memory in use, as the total less what the kernel says is available.
pub fn parse_meminfo(s: &str) -> Option<u32> {
    let field = |name: &str| {
        s.lines()
            .find_map(|l| l.strip_prefix(name))
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|kb| kb.parse::<u64>().ok())
    };
    let total = field("MemTotal:")?;
    let free = field("MemAvailable:").or_else(|| field("MemFree:"))?;
    Some((total.saturating_sub(free) / 1024) as u32)
}

pub fn temp_text(s: &Stats) -> String {
    s.temp_c.map_or("--".into(), |t| format!("{t:.1}°C"))
}

pub fn cpu_text(s: &Stats) -> String {
    s.cpu_mhz.map_or("--".into(), |m| format!("{m} MHz"))
}

pub fn ram_text(s: &Stats) -> String {
    s.ram_used_mb.map_or("--".into(), |m| format!("{m} MB"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn millidegrees_become_degrees() {
        assert_eq!(parse_temp("44700\n"), Some(44.7));
        assert_eq!(parse_temp("45"), Some(45.0));
        assert_eq!(parse_temp("hot"), None);
    }

    #[test]
    fn the_clock_reads_in_megahertz() {
        assert_eq!(parse_khz("2000000\n"), Some(2000));
        assert_eq!(parse_khz(""), None);
    }

    #[test]
    fn memory_in_use_is_total_less_available() {
        let info = "MemTotal:        1009000 kB\nMemFree:          100000 kB\nMemAvailable:     757096 kB\n";
        assert_eq!(parse_meminfo(info), Some(246));
        let old = "MemTotal: 2048 kB\nMemFree: 1024 kB\n";
        assert_eq!(parse_meminfo(old), Some(1));
        assert_eq!(parse_meminfo("nothing"), None);
    }

    #[test]
    fn a_tree_without_the_files_reads_as_unknown() {
        let s = Stats::read_from(Path::new("/nonexistent"));
        assert_eq!(s, Stats::default());
        assert_eq!(temp_text(&s), "--");
    }

    #[test]
    fn a_fake_tree_is_read() {
        let dir = std::env::temp_dir().join(format!("slot-sysinfo-{}", std::process::id()));
        let put = |p: &str, v: &str| {
            let f = dir.join(p.trim_start_matches('/'));
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, v).unwrap();
        };
        put(THERMAL, "44700");
        put(CPU_FREQ, "1200000");
        put(MEMINFO, "MemTotal: 1048576 kB\nMemAvailable: 524288 kB\n");
        let s = Stats::read_from(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(cpu_text(&s), "1200 MHz");
        assert_eq!(ram_text(&s), "512 MB");
        assert_eq!(temp_text(&s), "44.7°C");
    }
}
