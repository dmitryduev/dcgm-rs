use libloading::{Library, Symbol};
use std::{
    ffi::{c_char, CString},
    ptr,
};
use thiserror::Error;

pub mod dcgm_types;
pub mod metrics;

#[derive(Error, Debug)]
pub enum DcgmError {
    #[error("Library loading error: {0}")]
    LibraryError(#[from] libloading::Error),

    #[error("DCGM API returned error: {0} ({1})")]
    ApiError(i32, String),

    #[error("Invalid handle")]
    InvalidHandle,

    #[error("Connection failed")]
    ConnectionFailed,

    #[error("Field value error: {0}")]
    FieldValueError(String),

    #[error("Requires root: {0}")]
    RequiresRoot(String),
}

pub type Result<T> = std::result::Result<T, DcgmError>;

pub struct DcgmHandle {
    handle: u64,
    lib: Library,
}

impl DcgmHandle {
    pub fn new() -> Result<Self> {
        let lib = unsafe { Library::new("libdcgm.so.4") }?;

        let dcgm_init: Symbol<unsafe extern "C" fn() -> i32> = unsafe { lib.get(b"dcgmInit")? };

        let result = unsafe { dcgm_init() };
        if result != 0 {
            return Err(DcgmError::ApiError(result, "dcgmInit failed".to_string()));
        }

        // Start embedded mode
        let dcgm_start_embedded: Symbol<unsafe extern "C" fn(i32, *mut u64) -> i32> =
            unsafe { lib.get(b"dcgmStartEmbedded")? };

        let mut handle: u64 = 0;
        let result = unsafe { dcgm_start_embedded(1, &mut handle) }; // 1 = AUTO mode
        if result != 0 {
            return Err(DcgmError::ApiError(
                result,
                "dcgmStartEmbedded failed".to_string(),
            ));
        }

        // Update all fields immediately
        let dcgm_update_all_fields: Symbol<unsafe extern "C" fn(u64, i32) -> i32> =
            unsafe { lib.get(b"dcgmUpdateAllFields")? };

        let result = unsafe { dcgm_update_all_fields(handle, 1) }; // Wait for update
        if result != 0 {
            eprintln!("Warning: dcgmUpdateAllFields failed with code {}", result);
        }

        Ok(DcgmHandle { handle, lib })
    }

    pub fn with_connection(hostname: &str, port: Option<u16>) -> Result<Self> {
        let lib = unsafe { Library::new("libdcgm.so.4") }?;

        let dcgm_init: Symbol<unsafe extern "C" fn() -> i32> = unsafe { lib.get(b"dcgmInit")? };

        let result = unsafe { dcgm_init() };
        if result != 0 {
            return Err(DcgmError::ApiError(result, "dcgmInit failed".to_string()));
        }

        // Connect to remote hostengine
        let dcgm_connect: Symbol<unsafe extern "C" fn(*const i8, *mut u64) -> i32> =
            unsafe { lib.get(b"dcgmConnect")? };

        let addr_string = match port {
            Some(p) => format!("{}:{}", hostname, p),
            None => hostname.to_string(),
        };

        let c_addr = CString::new(addr_string).unwrap();
        let mut handle: u64 = 0;
        let result = unsafe { dcgm_connect(c_addr.as_ptr(), &mut handle) };
        if result != 0 {
            return Err(DcgmError::ConnectionFailed);
        }

        Ok(DcgmHandle { handle, lib })
    }

    // Force an update of all watched fields
    pub fn update_all_fields(&self, wait_for_update: bool) -> Result<()> {
        let dcgm_update_all_fields: Symbol<unsafe extern "C" fn(u64, i32) -> i32> =
            unsafe { self.lib.get(b"dcgmUpdateAllFields")? };

        let wait_flag = if wait_for_update { 1 } else { 0 };
        let result = unsafe { dcgm_update_all_fields(self.handle, wait_flag) };

        if result != 0 {
            return Err(DcgmError::ApiError(
                result,
                "dcgmUpdateAllFields failed".to_string(),
            ));
        }

        Ok(())
    }

    pub fn get_device_count(&self) -> Result<i32> {
        let dcgm_get_all_devices: Symbol<unsafe extern "C" fn(u64, *mut u32, *mut i32) -> i32> =
            unsafe { self.lib.get(b"dcgmGetAllDevices")? };

        let mut gpu_ids: [u32; 32] = [0; 32]; // DCGM_MAX_NUM_DEVICES
        let mut count: i32 = 0;

        let result = unsafe { dcgm_get_all_devices(self.handle, gpu_ids.as_mut_ptr(), &mut count) };
        if result != 0 {
            return Err(DcgmError::ApiError(
                result,
                "dcgmGetAllDevices failed".to_string(),
            ));
        }

        Ok(count)
    }

    pub fn get_device_ids(&self) -> Result<Vec<u32>> {
        let dcgm_get_all_devices: Symbol<unsafe extern "C" fn(u64, *mut u32, *mut i32) -> i32> =
            unsafe { self.lib.get(b"dcgmGetAllDevices")? };

        let mut gpu_ids: [u32; 32] = [0; 32]; // DCGM_MAX_NUM_DEVICES
        let mut count: i32 = 0;

        let result = unsafe { dcgm_get_all_devices(self.handle, gpu_ids.as_mut_ptr(), &mut count) };
        if result != 0 {
            return Err(DcgmError::ApiError(
                result,
                "dcgmGetAllDevices failed".to_string(),
            ));
        }

        Ok(gpu_ids[0..count as usize].to_vec())
    }

    // Get device name (like "Tesla T4")
    pub fn get_device_name(&self, device_id: u32) -> Result<String> {
        use crate::dcgm_types::{DCGM_FE_GPU, DCGM_FI_DEV_NAME, DCGM_FV_FLAG_LIVE_DATA};

        let field_values = self.get_device_field_values(device_id, &[DCGM_FI_DEV_NAME], true)?;

        if field_values.is_empty() {
            return Err(DcgmError::FieldValueError(
                "No device name data returned".to_string(),
            ));
        }

        let field_value = &field_values[0];
        let c_str = unsafe { &field_value.value.str };

        // Convert C string to Rust string
        let mut name = String::new();
        let mut i = 0;
        while i < c_str.len() && c_str[i] != 0 {
            name.push(c_str[i] as u8 as char);
            i += 1;
        }

        Ok(name)
    }

    // Get device field values helper function
    pub(crate) fn get_device_field_values(
        &self,
        device_id: u32,
        field_ids: &[u16],
        use_live_data: bool,
    ) -> Result<Vec<dcgm_types::DcgmFieldValue>> {
        use crate::dcgm_types::{DcgmFieldValue, DCGM_FE_GPU, DCGM_FV_FLAG_LIVE_DATA};

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

impl Drop for DcgmHandle {
    fn drop(&mut self) {
        if self.handle != 0 {
            // Try to clean up
            unsafe {
                // Stop embedded mode if we're in it
                if let Ok(dcgm_stop_embedded) = self
                    .lib
                    .get::<unsafe extern "C" fn(u64) -> i32>(b"dcgmStopEmbedded")
                {
                    let _ = dcgm_stop_embedded(self.handle);
                }

                // Finally shut down DCGM
                if let Ok(dcgm_shutdown) = self
                    .lib
                    .get::<unsafe extern "C" fn() -> i32>(b"dcgmShutdown")
                {
                    let _ = dcgm_shutdown();
                }
            }
        }
    }
}

#[repr(C)]
struct EntityPair {
    pub entity_group_id: u32,
    pub entity_id: u32,
}
