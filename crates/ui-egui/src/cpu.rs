//! The control bar's CPU readout: this process's share of the whole machine, read about once a
//! second. The web build has no process list, so it never reports a share.

/// Seconds between two reads. sysinfo turns CPU time into a share only across two reads, so the
/// first reading is near zero and the next ones are meaningful.
const INTERVAL: f64 = 1.0;

/// Reads this process's CPU share at most once per [`INTERVAL`].
#[derive(Default)]
pub struct CpuMeter {
    /// The egui time of the last read, and the share it gave.
    sampled_at: Option<f64>,
    percent: Option<f32>,
    #[cfg(not(target_arch = "wasm32"))]
    system: Option<sysinfo::System>,
}

impl CpuMeter {
    /// The share of the machine's CPU in percent, or `None` where it cannot be read. `now` is
    /// egui's input time in seconds.
    pub fn sample(&mut self, now: f64) -> Option<f32> {
        if self.sampled_at.is_some_and(|at| now - at < INTERVAL) {
            return self.percent;
        }
        self.sampled_at = Some(now);
        self.percent = self.read();
        self.percent
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn read(&mut self) -> Option<f32> {
        use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate};
        let system = self.system.get_or_insert_with(sysinfo::System::new);
        let pid = Pid::from_u32(std::process::id());
        system.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, ProcessRefreshKind::nothing().with_cpu());
        // A process reports up to 100% per core; divide by the cores for a share of the machine.
        let cores = std::thread::available_parallelism().map_or(1.0, |n| n.get() as f32);
        system.process(pid).map(|process| (process.cpu_usage() / cores).clamp(0.0, 100.0))
    }

    #[cfg(target_arch = "wasm32")]
    fn read(&mut self) -> Option<f32> {
        None
    }
}
