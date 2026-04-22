use tracing::debug;

use crate::metrics::prometheus::{MEMORY_USAGE_BYTES, THREAD_COUNT, UPTIME_SECONDS, UPTIME};

/// Update system metrics (memory, threads, etc.)
pub fn update_system_metrics() {
    // Read memory usage from /proc/self/status
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                if let Some(bytes_str) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = bytes_str.parse::<u64>() {
                        MEMORY_USAGE_BYTES.set((kb * 1024) as i64);
                    }
                }
                break;
            }
        }
    }
    
    // Update uptime
    UPTIME_SECONDS.set(UPTIME.get() as i64);
    
    // Thread count (approximate)
    THREAD_COUNT.set(0); // Will be updated by tokio runtime
}
