use crate::dcgm_types::{
    is_fp64_blank, is_int64_blank, DcgmFieldValue, DCGM_CLOCKS_EVENT_REASON_CLOCKS_SETTING,
    DCGM_CLOCKS_EVENT_REASON_GPU_IDLE, DCGM_CLOCKS_EVENT_REASON_HW_POWER_BRAKE,
    DCGM_CLOCKS_EVENT_REASON_HW_SLOWDOWN, DCGM_CLOCKS_EVENT_REASON_HW_THERMAL,
    DCGM_CLOCKS_EVENT_REASON_SW_POWER_CAP, DCGM_CLOCKS_EVENT_REASON_SW_THERMAL,
    DCGM_FI_DEV_CLOCKS_EVENT_REASONS, DCGM_FI_DEV_ENFORCED_POWER_LIMIT, DCGM_FI_DEV_FB_FREE,
    DCGM_FI_DEV_FB_TOTAL, DCGM_FI_DEV_FB_USED, DCGM_FI_DEV_GPU_MAX_OP_TEMP, DCGM_FI_DEV_GPU_TEMP,
    DCGM_FI_DEV_GPU_UTIL, DCGM_FI_DEV_MEM_CLOCK, DCGM_FI_DEV_POWER_USAGE,
    DCGM_FI_DEV_POWER_VIOLATION, DCGM_FI_DEV_SM_CLOCK, DCGM_FI_DEV_THERMAL_VIOLATION,
    DCGM_FI_DEV_TOTAL_ENERGY_CONSUMPTION,
};
use crate::{DcgmError, DcgmHandle, Result};
use std::collections::HashMap;

/// Basic GPU metrics that should be accessible without root
#[derive(Debug, Clone)]
pub struct GpuMetrics {
    pub device_id: u32,
    pub timestamp: i64,
    // Power metrics
    pub power_usage: Option<f64>,          // in Watts
    pub energy_consumption: Option<i64>,   // in mJ
    pub enforced_power_limit: Option<i64>, // in W
    pub power_violation_time: Option<i64>, // in µs
    // Temperature metrics
    pub gpu_temp: Option<i64>,               // in °C
    pub max_gpu_temp: Option<i64>,           // in °C
    pub thermal_violation_time: Option<i64>, // in µs
    // Memory metrics
    pub fb_total: Option<i64>, // in MB
    pub fb_free: Option<i64>,  // in MB
    pub fb_used: Option<i64>,  // in MB
    // Utilization metrics
    pub gpu_util: Option<i64>, // in %
    // Clock metrics
    pub sm_clock: Option<i64>,  // in MHz
    pub mem_clock: Option<i64>, // in MHz
    // Throttling reasons (bitmask)
    pub clock_throttle_reasons: Option<u64>,   // bitmask
    pub throttle_reasons: Option<Vec<String>>, // human-readable reasons
}

impl GpuMetrics {
    fn new(device_id: u32) -> Self {
        GpuMetrics {
            device_id,
            timestamp: 0,
            power_usage: None,
            energy_consumption: None,
            enforced_power_limit: None,
            power_violation_time: None,
            gpu_temp: None,
            max_gpu_temp: None,
            thermal_violation_time: None,
            fb_total: None,
            fb_free: None,
            fb_used: None,
            gpu_util: None,
            sm_clock: None,
            mem_clock: None,
            clock_throttle_reasons: None,
            throttle_reasons: None,
        }
    }

    fn decode_throttle_reasons(&mut self) {
        if let Some(reasons) = self.clock_throttle_reasons {
            let mut decoded = Vec::new();

            if reasons & DCGM_CLOCKS_EVENT_REASON_GPU_IDLE != 0 {
                decoded.push("GPU_IDLE".to_string());
            }
            if reasons & DCGM_CLOCKS_EVENT_REASON_CLOCKS_SETTING != 0 {
                decoded.push("CLOCKS_SETTING".to_string());
            }
            if reasons & DCGM_CLOCKS_EVENT_REASON_SW_POWER_CAP != 0 {
                decoded.push("SW_POWER_CAP".to_string());
            }
            if reasons & DCGM_CLOCKS_EVENT_REASON_HW_SLOWDOWN != 0 {
                decoded.push("HW_SLOWDOWN".to_string());
            }
            if reasons & DCGM_CLOCKS_EVENT_REASON_SW_THERMAL != 0 {
                decoded.push("SW_THERMAL".to_string());
            }
            if reasons & DCGM_CLOCKS_EVENT_REASON_HW_THERMAL != 0 {
                decoded.push("HW_THERMAL".to_string());
            }
            if reasons & DCGM_CLOCKS_EVENT_REASON_HW_POWER_BRAKE != 0 {
                decoded.push("HW_POWER_BRAKE".to_string());
            }

            self.throttle_reasons = Some(decoded);
        }
    }
}

impl DcgmHandle {
    /// Get basic GPU metrics that should be accessible without root
    pub fn get_basic_metrics(&self, device_id: u32) -> Result<GpuMetrics> {
        // Force an update to get the latest values
        self.update_all_fields(true)?;

        // List of field IDs we want to query
        let field_ids = [
            DCGM_FI_DEV_POWER_USAGE,
            DCGM_FI_DEV_TOTAL_ENERGY_CONSUMPTION,
            DCGM_FI_DEV_GPU_TEMP,
            DCGM_FI_DEV_GPU_MAX_OP_TEMP,
            DCGM_FI_DEV_ENFORCED_POWER_LIMIT,
            DCGM_FI_DEV_FB_TOTAL,
            DCGM_FI_DEV_FB_FREE,
            DCGM_FI_DEV_FB_USED,
            DCGM_FI_DEV_GPU_UTIL,
            DCGM_FI_DEV_SM_CLOCK,
            DCGM_FI_DEV_MEM_CLOCK,
            DCGM_FI_DEV_POWER_VIOLATION,
            DCGM_FI_DEV_THERMAL_VIOLATION,
            DCGM_FI_DEV_CLOCKS_EVENT_REASONS,
        ];

        // Get the field values with the LIVE flag to ensure we get the current data
        let field_values = self.get_device_field_values(device_id, &field_ids, true)?;

        if field_values.is_empty() {
            return Err(DcgmError::FieldValueError(
                "No metrics data returned".to_string(),
            ));
        }

        // Create a metrics object to hold the results
        let mut metrics = GpuMetrics::new(device_id);

        // The latest timestamp we find - use as overall timestamp
        let mut latest_timestamp = 0;

        // Process the field values
        for field_value in field_values {
            // Update the latest timestamp
            if field_value.timestamp > latest_timestamp {
                latest_timestamp = field_value.timestamp;
            }

            match field_value.field_id {
                DCGM_FI_DEV_POWER_USAGE => {
                    let value = unsafe { field_value.value.dbl };
                    if !is_fp64_blank(value) {
                        metrics.power_usage = Some(value);
                    }
                }
                DCGM_FI_DEV_TOTAL_ENERGY_CONSUMPTION => {
                    let value = unsafe { field_value.value.i64 };
                    if !is_int64_blank(value) {
                        metrics.energy_consumption = Some(value);
                    }
                }
                DCGM_FI_DEV_GPU_TEMP => {
                    let value = unsafe { field_value.value.i64 };
                    if !is_int64_blank(value) {
                        metrics.gpu_temp = Some(value);
                    }
                }
                DCGM_FI_DEV_GPU_MAX_OP_TEMP => {
                    let value = unsafe { field_value.value.i64 };
                    if !is_int64_blank(value) {
                        metrics.max_gpu_temp = Some(value);
                    }
                }
                DCGM_FI_DEV_ENFORCED_POWER_LIMIT => {
                    let value = unsafe { field_value.value.i64 };
                    if !is_int64_blank(value) {
                        metrics.enforced_power_limit = Some(value);
                    }
                }
                DCGM_FI_DEV_FB_TOTAL => {
                    let value = unsafe { field_value.value.i64 };
                    if !is_int64_blank(value) {
                        metrics.fb_total = Some(value);
                    }
                }
                DCGM_FI_DEV_FB_FREE => {
                    let value = unsafe { field_value.value.i64 };
                    if !is_int64_blank(value) {
                        metrics.fb_free = Some(value);
                    }
                }
                DCGM_FI_DEV_FB_USED => {
                    let value = unsafe { field_value.value.i64 };
                    if !is_int64_blank(value) {
                        metrics.fb_used = Some(value);
                    }
                }
                DCGM_FI_DEV_GPU_UTIL => {
                    let value = unsafe { field_value.value.i64 };
                    if !is_int64_blank(value) {
                        metrics.gpu_util = Some(value);
                    }
                }
                DCGM_FI_DEV_SM_CLOCK => {
                    let value = unsafe { field_value.value.i64 };
                    if !is_int64_blank(value) {
                        metrics.sm_clock = Some(value);
                    }
                }
                DCGM_FI_DEV_MEM_CLOCK => {
                    let value = unsafe { field_value.value.i64 };
                    if !is_int64_blank(value) {
                        metrics.mem_clock = Some(value);
                    }
                }
                DCGM_FI_DEV_POWER_VIOLATION => {
                    let value = unsafe { field_value.value.i64 };
                    if !is_int64_blank(value) {
                        metrics.power_violation_time = Some(value);
                    }
                }
                DCGM_FI_DEV_THERMAL_VIOLATION => {
                    let value = unsafe { field_value.value.i64 };
                    if !is_int64_blank(value) {
                        metrics.thermal_violation_time = Some(value);
                    }
                }
                DCGM_FI_DEV_CLOCKS_EVENT_REASONS => {
                    let value = unsafe { field_value.value.i64 };
                    if !is_int64_blank(value) {
                        metrics.clock_throttle_reasons = Some(value as u64);
                    }
                }
                _ => {}
            }
        }

        metrics.timestamp = latest_timestamp;

        // Decode throttle reasons
        metrics.decode_throttle_reasons();

        Ok(metrics)
    }
}
