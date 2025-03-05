use dcgm_rs::DcgmHandle;
use std::{thread, time::Duration};

fn main() {
    // Create a new DCGM handle (embedded mode)
    let dcgm = match DcgmHandle::new() {
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

    // Monitor power usage and SM activity for each GPU
    for _ in 0..10 {
        for &gpu_id in &gpu_ids {
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

        // Wait a second before the next measurement
        thread::sleep(Duration::from_secs(1));
    }
}
