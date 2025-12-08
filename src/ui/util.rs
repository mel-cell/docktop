use crate::docker::ContainerStats;

pub fn calculate_cpu_usage(stats: &ContainerStats, previous_stats: &Option<ContainerStats>) -> f64 {
    let mut cpu_percent = 0.0;
    
    if let Some(prev) = previous_stats {
        let cpu_delta = stats.cpu_stats.cpu_usage.total_usage as f64 - prev.cpu_stats.cpu_usage.total_usage as f64;
        let system_delta = stats.cpu_stats.system_cpu_usage.unwrap_or(0) as f64 - prev.cpu_stats.system_cpu_usage.unwrap_or(0) as f64;
        
        if system_delta > 0.0 && cpu_delta > 0.0 {
            let percpu_len = stats.cpu_stats.cpu_usage.percpu_usage.as_ref().map(|v| v.len()).unwrap_or(1) as f64;
            cpu_percent = (cpu_delta / system_delta) * percpu_len * 100.0;
        }
    }
    
    cpu_percent
}
