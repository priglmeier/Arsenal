//! CPU and RAM usage of the Arma server process and of the whole system.
//!
//! Numbers are read with the `sysinfo` crate (Windows and Linux) about once a second.
//! All values are percentages of the whole machine, like Task Manager shows them: a
//! server using one of eight cores is 12.5% CPU.

use std::time::{Duration, Instant};

use sysinfo::{Pid, ProcessesToUpdate, System};

const REFRESH_EVERY: Duration = Duration::from_secs(1);

/// Color bands. CPU is a momentary spike, RAM is a filling bucket, so the limits differ.
pub const CPU_WARNING: f32 = 70.0;
pub const CPU_CRITICAL: f32 = 85.0;
pub const RAM_WARNING: f32 = 75.0;
pub const RAM_CRITICAL: f32 = 90.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// Green.
    Normal,
    /// Yellow.
    Warning,
    /// Red.
    Critical,
}

/// Up to and including `warning` is normal, above `critical` is critical.
fn rate(value: f32, warning: f32, critical: f32) -> Level {
    if value > critical {
        Level::Critical
    } else if value > warning {
        Level::Warning
    } else {
        Level::Normal
    }
}

pub fn cpu_level(percent: f32) -> Level {
    rate(percent, CPU_WARNING, CPU_CRITICAL)
}

pub fn ram_level(percent: f32) -> Level {
    rate(percent, RAM_WARNING, RAM_CRITICAL)
}

/// One reading: CPU and RAM as a percentage of the machine, plus the RAM in bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Usage {
    pub cpu: f32,
    pub ram: f32,
    pub ram_bytes: u64,
}

pub struct Metrics {
    sys: System,
    cpu_count: f32,
    last_refresh: Option<Instant>,
    pub total_ram_bytes: u64,
    pub system: Usage,
    /// `None` while the server is not running.
    pub server: Option<Usage>,
}

impl Metrics {
    pub fn new() -> Self {
        let cpus = std::thread::available_parallelism().map_or(1, |n| n.get());
        Self {
            sys: System::new(),
            cpu_count: cpus as f32,
            last_refresh: None,
            total_ram_bytes: 0,
            system: Usage::default(),
            server: None,
        }
    }

    /// Takes a new reading when a second has passed since the last one. `server_pid` is the
    /// process id of the running server, if there is one.
    pub fn update(&mut self, server_pid: Option<u32>) {
        if let Some(last) = self.last_refresh {
            if last.elapsed() < REFRESH_EVERY {
                return;
            }
        }
        self.last_refresh = Some(Instant::now());

        // CPU usage is measured between two refreshes, so the very first reading is 0.
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        let total = self.sys.total_memory();
        let used = self.sys.used_memory();
        self.total_ram_bytes = total;
        self.system = Usage {
            cpu: self.sys.global_cpu_usage().clamp(0.0, 100.0),
            ram: percent(used, total),
            ram_bytes: used,
        };

        self.server = server_pid.and_then(|pid| {
            let pid = Pid::from_u32(pid);
            self.sys
                .refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
            let process = self.sys.process(pid)?;
            Some(Usage {
                // sysinfo reports 100% per core; divide to get a share of the whole machine.
                cpu: (process.cpu_usage() / self.cpu_count).clamp(0.0, 100.0),
                ram: percent(process.memory(), total),
                ram_bytes: process.memory(),
            })
        });
    }
}

fn percent(part: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        (part as f64 / total as f64 * 100.0).clamp(0.0, 100.0) as f32
    }
}

/// `1536 MB`-style text for a size in bytes: MB below 1 GB, otherwise GB with one decimal.
pub fn bytes_text(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else {
        format!("{:.0} MB", b / MB)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_bands_follow_the_table() {
        assert_eq!(cpu_level(0.0), Level::Normal);
        assert_eq!(cpu_level(70.0), Level::Normal);
        assert_eq!(cpu_level(70.01), Level::Warning);
        assert_eq!(cpu_level(85.0), Level::Warning);
        assert_eq!(cpu_level(85.01), Level::Critical);
        assert_eq!(cpu_level(100.0), Level::Critical);
    }

    #[test]
    fn ram_bands_follow_the_table() {
        assert_eq!(ram_level(0.0), Level::Normal);
        assert_eq!(ram_level(75.0), Level::Normal);
        assert_eq!(ram_level(75.01), Level::Warning);
        assert_eq!(ram_level(90.0), Level::Warning);
        assert_eq!(ram_level(90.01), Level::Critical);
        assert_eq!(ram_level(100.0), Level::Critical);
    }

    #[test]
    fn percent_handles_zero_and_overflow() {
        assert_eq!(percent(5, 0), 0.0);
        assert_eq!(percent(50, 200), 25.0);
        assert_eq!(percent(300, 200), 100.0);
    }

    #[test]
    fn sizes_are_readable() {
        assert_eq!(bytes_text(512 * 1024 * 1024), "512 MB");
        assert_eq!(bytes_text(3 * 1024 * 1024 * 1024 / 2), "1.5 GB");
    }
}
