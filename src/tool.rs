use std::{collections::VecDeque, time::Duration};
use sysinfo::{Pid, System};

pub fn spawn_mem_logger(interval_secs: u64) {
    tokio::spawn(async move {
        let pid = Pid::from_u32(std::process::id());
        let mut sys = System::new_all();
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));

        let mut rss_samples_bytes: VecDeque<u64> = VecDeque::new();
        const MAX_SAMPLES: usize = 10;

        loop {
            ticker.tick().await;

            sys.refresh_processes();

            if let Some(proc_) = sys.process(pid) {
                let rss_bytes = proc_.memory();
                let virt_bytes = proc_.virtual_memory();

                rss_samples_bytes.push_back(rss_bytes);
                if rss_samples_bytes.len() > MAX_SAMPLES {
                    rss_samples_bytes.pop_front();
                }

                let avg_rss_bytes =
                    rss_samples_bytes.iter().copied().sum::<u64>() / rss_samples_bytes.len() as u64;

                let rss_mib = rss_bytes as f64 / (1024.0 * 1024.0);
                let avg_rss_mib = avg_rss_bytes as f64 / (1024.0 * 1024.0);
                let virt_mib = virt_bytes as f64 / (1024.0 * 1024.0);

                println!(
                    "[mem] rss={:.2} MiB avg_rss={:.2} MiB virt={:.2} MiB samples={}",
                    rss_mib,
                    avg_rss_mib,
                    virt_mib,
                    rss_samples_bytes.len()
                );
            }
        }
    });
}
