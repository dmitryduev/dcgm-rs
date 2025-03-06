use dcgm_rs::{DcgmError, DcgmHandle};
use std::{thread, time::Duration};

fn main() {
    // Create a new DCGM handle (embedded mode)
    let mut dcgm = match DcgmHandle::new() {
        Ok(handle) => handle,
        Err(e) => {
            eprintln!("Failed to initialize DCGM: {}", e);
            return;
        }
    };

    // Get available GPU IDs
    let gpu_ids = match dcgm.get_device_ids() {
        Ok(ids) => ids,
        Err(e) => {
            eprintln!("Failed to get GPU IDs: {}", e);
            return;
        }
    };

    println!("Found {} GPUs", gpu_ids.len());

    // Enable watches for our metrics
    if let Err(e) = dcgm.enable_power_metrics() {
        eprintln!("Warning: Failed to enable power metrics: {}", e);
    }

    // Try to enable profiling metrics, but handle the case where it requires root
    let profiling_enabled = match dcgm.enable_profiling_metrics() {
        Ok(_) => true,
        Err(e) => {
            eprintln!("Warning: Failed to enable profiling metrics: {}", e);

            match e {
                DcgmError::RequiresRoot(_) => {
                    println!("Note: Profiling metrics require root access. Only power usage will be shown.");
                    println!("Try running with 'sudo' to access SM activity metrics.");
                    false
                }
                _ => false,
            }
        }
    };

    // Wait a moment for metrics to initialize
    thread::sleep(Duration::from_millis(500));

    // Force an update before we start monitoring
    if let Err(e) = dcgm.update_all_fields(true) {
        eprintln!("Warning: Failed to update fields: {}", e);
    }

    // Monitor power usage and SM activity for each GPU
    for _ in 0..10 {
        // Force an update to get fresh data
        if let Err(e) = dcgm.update_all_fields(true) {
            eprintln!("Warning: Failed to update fields: {}", e);
        }

        for &gpu_id in &gpu_ids {
            // If profiling metrics aren't available, just show power usage
            if !profiling_enabled {
                match dcgm.get_power_usage(gpu_id) {
                    Ok(power) => {
                        println!(
                            "GPU {}: Power usage: {:.2} W (SM Activity: Not available)",
                            gpu_id, power.power_usage
                        );
                    }
                    Err(e) => {
                        eprintln!("Failed to get power usage for GPU {}: {}", gpu_id, e);
                    }
                }
            } else {
                // If profiling is enabled, try to get both metrics
                match dcgm.get_metrics(gpu_id) {
                    Ok((power, sm_activity)) => {
                        println!(
                            "GPU {}: Power usage: {:.2} W, SM Activity: {:.2}%",
                            gpu_id,
                            power.power_usage,
                            sm_activity.sm_active * 100.0
                        );
                    }
                    Err(e) => {
                        eprintln!("Failed to get metrics for GPU {}: {}", gpu_id, e);
                    }
                }
            }
        }

        // Wait a second before the next measurement
        thread::sleep(Duration::from_secs(1));
    }
}
