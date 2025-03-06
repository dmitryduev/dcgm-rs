# dcgm-rs

A safe Rust wrapper for NVIDIA's Data Center GPU Manager (DCGM) library.

## Features

- Safe Rust API to interact with DCGM
- Low-level bindings using libloading
- Metrics collection for GPU power usage and SM activity
- Support for both embedded and standalone DCGM hostengine

## Requirements

- NVIDIA GPU with DCGM support
- DCGM installed (typically via the NVIDIA datacenter repo)
- Rust 2021 edition

## Usage

```rust
use dcgm_rs::DcgmHandle;

fn main() {
    // Create a DCGM handle using embedded mode
    let dcgm = DcgmHandle::new().expect("Failed to initialize DCGM");

    // Get all available GPU IDs
    let gpu_ids = dcgm.get_device_ids().expect("Failed to get GPU IDs");

    // For each GPU, get power usage and SM activity
    for &gpu_id in &gpu_ids {
        // Get both metrics at once
        if let Ok((power, sm_activity)) = dcgm.get_metrics(gpu_id) {
            println!(
                "GPU {}: Power usage: {:.2} W, SM Activity: {:.2}%",
                gpu_id,
                power.power_usage,
                sm_activity.sm_active * 100.0
            );
        }

        // Or get metrics individually
        if let Ok(power) = dcgm.get_power_usage(gpu_id) {
            println!("GPU {}: Power usage: {:.2} W", gpu_id, power.power_usage);
        }

        if let Ok(sm) = dcgm.get_sm_activity(gpu_id) {
            println!("GPU {}: SM Activity: {:.2}%", gpu_id, sm.sm_active * 100.0);
        }
    }
}
```

## Connecting to a Remote DCGM Hostengine

```rust
// Connect to a remote DCGM hostengine
let dcgm = DcgmHandle::with_connection("hostname", Some(5555))
    .expect("Failed to connect to DCGM hostengine");

// Use the handle as normal
let gpu_ids = dcgm.get_device_ids().expect("Failed to get GPU IDs");
```

## Adding More Metrics

This library currently supports two metrics:

- Power usage (DCGM_FI_DEV_POWER_USAGE)
- SM activity (DCGM_FI_PROF_SM_ACTIVE)

Additional metrics can be added by:

1. Adding the field ID constant in `dcgm_types.rs`
2. Creating a struct to represent the metric
3. Implementing a method in the `DcgmHandle` to fetch the metric

## License

This project is licensed under the MIT License.
