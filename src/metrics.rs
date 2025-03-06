use crate::dcgm_types::{
    is_fp64_blank, is_int64_blank, DcgmFieldValue, DCGM_FE_GPU, DCGM_FI_DEV_POWER_USAGE,
    DCGM_FI_PROF_SM_ACTIVE, DCGM_FT_DOUBLE, DCGM_FT_INT64, DCGM_FV_FLAG_LIVE_DATA,
};
use crate::{DcgmError, DcgmHandle, Result};
use libloading::Symbol;
use std::ffi::CString;

/// A struct representing the power usage of a GPU
#[derive(Debug, Clone, Copy)]
pub struct PowerUsage {
    pub device_id: u32,
    pub power_usage: f64, // in Watts
    pub timestamp: i64,   // in microseconds since 1970
}

/// A struct representing the SM activity of a GPU
#[derive(Debug, Clone, Copy)]
pub struct SmActivity {
    pub device_id: u32,
    pub sm_active: f64, // ratio (0.0 - 1.0)
    pub timestamp: i64, // in microseconds since 1970
}

impl DcgmHandle {
    /// Get the latest power usage for a specific GPU
    pub fn get_power_usage(&mut self, device_id: u32) -> Result<PowerUsage> {
        // Ensure power metrics are being watched
        self.enable_power_metrics()?;

        // Force an update to get the latest values
        self.update_all_fields(true)?;

        // Get the field values with the LIVE flag to ensure we get the current data
        let field_values =
            self.get_device_field_values(device_id, &[DCGM_FI_DEV_POWER_USAGE], true)?;

        if field_values.is_empty() {
            return Err(DcgmError::FieldValueError(
                "No power usage data returned".to_string(),
            ));
        }

        let field_value = &field_values[0];
        let power_usage = unsafe { field_value.value.dbl };

        if is_fp64_blank(power_usage) {
            return Err(DcgmError::FieldValueError(
                "Power usage value is blank".to_string(),
            ));
        }

        Ok(PowerUsage {
            device_id,
            power_usage,
            timestamp: field_value.timestamp,
        })
    }

    /// Get the latest SM activity for a specific GPU
    pub fn get_sm_activity(&mut self, device_id: u32) -> Result<SmActivity> {
        // Ensure profiling metrics are being watched
        self.enable_profiling_metrics()?;

        // Force an update to get the latest values
        self.update_all_fields(true)?;

        // Get the field values with the LIVE flag to ensure we get the current data
        let field_values =
            self.get_device_field_values(device_id, &[DCGM_FI_PROF_SM_ACTIVE], true)?;

        if field_values.is_empty() {
            return Err(DcgmError::FieldValueError(
                "No SM activity data returned".to_string(),
            ));
        }

        let field_value = &field_values[0];
        let sm_active = unsafe { field_value.value.dbl };

        if is_fp64_blank(sm_active) {
            return Err(DcgmError::FieldValueError(
                "SM activity value is blank".to_string(),
            ));
        }

        Ok(SmActivity {
            device_id,
            sm_active,
            timestamp: field_value.timestamp,
        })
    }

    /// Get both power usage and SM activity metrics for a specific GPU
    pub fn get_metrics(&mut self, device_id: u32) -> Result<(PowerUsage, SmActivity)> {
        // Ensure both metrics are being watched
        self.enable_power_metrics()?;
        self.enable_profiling_metrics()?;

        // Force an update to get the latest values
        self.update_all_fields(true)?;

        // Get the field values with the LIVE flag to ensure we get the current data
        let field_values = self.get_device_field_values(
            device_id,
            &[DCGM_FI_DEV_POWER_USAGE, DCGM_FI_PROF_SM_ACTIVE],
            true,
        )?;

        if field_values.len() < 2 {
            return Err(DcgmError::FieldValueError(
                "Incomplete metrics data returned".to_string(),
            ));
        }

        let mut power_usage = None;
        let mut sm_activity = None;

        for field_value in field_values {
            match field_value.field_id {
                DCGM_FI_DEV_POWER_USAGE => {
                    let value = unsafe { field_value.value.dbl };
                    if !is_fp64_blank(value) {
                        power_usage = Some(PowerUsage {
                            device_id,
                            power_usage: value,
                            timestamp: field_value.timestamp,
                        });
                    }
                }
                DCGM_FI_PROF_SM_ACTIVE => {
                    let value = unsafe { field_value.value.dbl };
                    if !is_fp64_blank(value) {
                        sm_activity = Some(SmActivity {
                            device_id,
                            sm_active: value,
                            timestamp: field_value.timestamp,
                        });
                    }
                }
                _ => {}
            }
        }

        match (power_usage, sm_activity) {
            (Some(p), Some(s)) => Ok((p, s)),
            (None, Some(_)) => Err(DcgmError::FieldValueError(
                "Power usage value is missing or blank".to_string(),
            )),
            (Some(_), None) => Err(DcgmError::FieldValueError(
                "SM activity value is missing or blank".to_string(),
            )),
            (None, None) => Err(DcgmError::FieldValueError(
                "Both power usage and SM activity values are missing or blank".to_string(),
            )),
        }
    }

    /// Get the latest values for specified fields for a device
    fn get_device_field_values(
        &self,
        device_id: u32,
        field_ids: &[u16],
        use_live_data: bool,
    ) -> Result<Vec<DcgmFieldValue>> {
        let dcgm_entities_get_latest_values: Symbol<
            unsafe extern "C" fn(
                handle: u64,
                entities: *const EntityPair,
                entity_count: u32,
                fields: *const u16,
                field_count: u32,
                flags: u32,
                values: *mut DcgmFieldValue,
            ) -> i32,
        > = unsafe { self.lib.get(b"dcgmEntitiesGetLatestValues")? };

        let entity = EntityPair {
            entity_group_id: DCGM_FE_GPU,
            entity_id: device_id,
        };

        let field_count = field_ids.len() as u32;
        let mut values: Vec<DcgmFieldValue> = Vec::with_capacity(field_count as usize);
        for _ in 0..field_count {
            values.push(DcgmFieldValue::default());
        }

        // Flag to request live data if needed
        let flags: u32 = if use_live_data {
            DCGM_FV_FLAG_LIVE_DATA
        } else {
            0
        };

        let result = unsafe {
            dcgm_entities_get_latest_values(
                self.handle,
                &entity,
                1,
                field_ids.as_ptr(),
                field_count,
                flags,
                values.as_mut_ptr(),
            )
        };

        if result != 0 {
            return Err(DcgmError::ApiError(
                result,
                "dcgmEntitiesGetLatestValues failed".to_string(),
            ));
        }

        Ok(values)
    }
}

#[repr(C)]
struct EntityPair {
    pub entity_group_id: u32,
    pub entity_id: u32,
}
