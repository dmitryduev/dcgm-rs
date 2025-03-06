use dcgm_rs::{DcgmError, DcgmHandle};
use std::{thread, time::Duration};

fn main() {
    // Create a new DCGM handle (embedded mode)
    // let dcgm = match DcgmHandle::new() {
    //     Ok(handle) => handle,
    //     Err(e) => {
    //         eprintln!("Failed to initialize DCGM: {}", e);
    //         return;
    //     }
    // };

    // Connect to local nv-hostengine at port 5555
    let dcgm = match DcgmHandle::with_connection("localhost", Some(5555)) {
        Ok(handle) => handle,
        Err(e) => {
            eprintln!("Failed to connect to DCGM: {}", e);
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

    // Print GPU names
    for &gpu_id in &gpu_ids {
        match dcgm.get_device_name(gpu_id) {
            Ok(name) => println!("GPU {}: {}", gpu_id, name),
            Err(e) => eprintln!("Failed to get name for GPU {}: {}", gpu_id, e),
        }
    }

    println!("\nMonitoring GPU metrics (press Ctrl+C to stop):");

    // Continuously monitor all metrics for 10 measurements
    for i in 1..=10 {
        println!("\nMeasurement #{}", i);

        for &gpu_id in &gpu_ids {
            match dcgm.get_basic_metrics(gpu_id) {
                Ok(metrics) => {
                    print!("GPU {} - ", gpu_id);

                    // Power metrics
                    if let Some(power) = metrics.power_usage {
                        print!("Power: {:.2} W, ", power);
                    }

                    // Temperature
                    if let Some(temp) = metrics.gpu_temp {
                        print!("Temp: {}°C", temp);
                        if let Some(max_temp) = metrics.max_gpu_temp {
                            print!("/{}°C max, ", max_temp);
                        } else {
                            print!(", ");
                        }
                    }

                    // Memory
                    if let Some(used) = metrics.fb_used {
                        if let Some(total) = metrics.fb_total {
                            print!(
                                "Mem: {}/{} MB ({:.1}%), ",
                                used,
                                total,
                                (used as f64 / total as f64) * 100.0
                            );
                        } else {
                            print!("Mem: {} MB, ", used);
                        }
                    }

                    // Utilization
                    if let Some(util) = metrics.gpu_util {
                        print!("Util: {}%, ", util);
                    }

                    // Clocks
                    if let Some(sm_clock) = metrics.sm_clock {
                        print!("SM: {} MHz, ", sm_clock);
                    }

                    if let Some(mem_clock) = metrics.mem_clock {
                        print!("Mem: {} MHz, ", mem_clock);
                    }

                    // Throttling reasons
                    if let Some(reasons) = &metrics.throttle_reasons {
                        if !reasons.is_empty() {
                            print!("Throttle: {}", reasons.join(", "));
                        }
                    }

                    println!();

                    // Print violations if they exist
                    if let Some(power_violation) = metrics.power_violation_time {
                        if power_violation > 0 {
                            println!("  Power violations: {} μs", power_violation);
                        }
                    }

                    if let Some(thermal_violation) = metrics.thermal_violation_time {
                        if thermal_violation > 0 {
                            println!("  Thermal violations: {} μs", thermal_violation);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to get metrics for GPU {}: {}", gpu_id, e);
                }
            }
        }

        // Wait before next measurement
        thread::sleep(Duration::from_secs(1));
    }
}
