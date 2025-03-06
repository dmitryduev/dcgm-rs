use libloading::{Library, Symbol};
use std::{ffi::CString, ptr, time::Duration};
use thiserror::Error;

pub mod dcgm_types;
pub mod metrics;

#[derive(Error, Debug)]
pub enum DcgmError {
    #[error("Library loading error: {0}")]
    LibraryError(#[from] libloading::Error),

    #[error("DCGM API returned error: {0} ({1})")]
    ApiError(i32, String),

    #[error("Failed to initialize DCGM")]
    InitializationError,

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
    // Track whether we've enabled watches for power and profiling metrics
    power_watched: bool,
    prof_watched: bool,
    power_group_id: u64,
    prof_group_id: u64,
}

impl DcgmHandle {
    pub fn new() -> Result<Self> {
        let lib = unsafe { Library::new("libdcgm.so") }?;

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

        // Update all fields initially to make sure we're getting fresh data
        let dcgm_update_all_fields: Symbol<unsafe extern "C" fn(u64, i32) -> i32> =
            unsafe { lib.get(b"dcgmUpdateAllFields")? };

        let result = unsafe { dcgm_update_all_fields(handle, 1) }; // Wait for update
        if result != 0 {
            eprintln!("Warning: dcgmUpdateAllFields failed with code {}", result);
        }

        Ok(DcgmHandle {
            handle,
            lib,
            power_watched: false,
            prof_watched: false,
            power_group_id: 0,
            prof_group_id: 0,
        })
    }

    pub fn with_connection(hostname: &str, port: Option<u16>) -> Result<Self> {
        let lib = unsafe { Library::new("libdcgm.so") }?;

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

        // Update all fields initially to make sure we're getting fresh data
        let dcgm_update_all_fields: Symbol<unsafe extern "C" fn(u64, i32) -> i32> =
            unsafe { lib.get(b"dcgmUpdateAllFields")? };

        let result = unsafe { dcgm_update_all_fields(handle, 1) }; // Wait for update
        if result != 0 {
            eprintln!("Warning: dcgmUpdateAllFields failed with code {}", result);
        }

        Ok(DcgmHandle {
            handle,
            lib,
            power_watched: false,
            prof_watched: false,
            power_group_id: 0,
            prof_group_id: 0,
        })
    }

    // Enable watching specific metrics for all GPUs
    pub fn enable_power_metrics(&mut self) -> Result<()> {
        if self.power_watched {
            return Ok(());
        }

        // Just use the field directly through dcgmEntitiesGetLatestValues
        // This avoids the field group creation which might be causing issues
        self.power_watched = true;
        Ok(())
    }

    // Enable watching profiling metrics (including SM activity)
    pub fn enable_profiling_metrics(&mut self) -> Result<()> {
        if self.prof_watched {
            return Ok(());
        }

        // For profiling metrics, we need to set up proper watching
        // This likely requires root permissions
        let dcgm_field_group_create: Symbol<
            unsafe extern "C" fn(
                handle: u64,
                num_field_ids: i32,
                field_ids: *mut u16,
                field_group_name: *const i8,
                field_group_id: *mut u64,
            ) -> i32,
        > = unsafe { self.lib.get(b"dcgmFieldGroupCreate")? };

        let field_ids = [crate::dcgm_types::DCGM_FI_PROF_SM_ACTIVE];
        let pid = std::process::id();
        let field_group_name = CString::new(format!("ProfMetrics{}", pid)).unwrap();
        let mut field_group_id: u64 = 0;

        let result = unsafe {
            dcgm_field_group_create(
                self.handle,
                field_ids.len() as i32,
                field_ids.as_ptr() as *mut u16,
                field_group_name.as_ptr(),
                &mut field_group_id,
            )
        };

        if result != 0 && result != -26
        /* DCGM_ST_DUPLICATE_KEY */
        {
            return Err(DcgmError::ApiError(
                result,
                "dcgmFieldGroupCreate failed for profiling metrics".to_string(),
            ));
        }

        self.prof_group_id = field_group_id;

        // Start watching this field group on all GPUs
        let dcgm_watch_fields: Symbol<
            unsafe extern "C" fn(
                handle: u64,
                group_id: u64,
                field_group_id: u64,
                update_freq: i64,
                max_keep_age: f64,
                max_keep_samples: i32,
            ) -> i32,
        > = unsafe { self.lib.get(b"dcgmWatchFields")? };

        let result = unsafe {
            dcgm_watch_fields(
                self.handle,
                0x7fffffff, // DCGM_GROUP_ALL_GPUS
                field_group_id,
                100000, // Update every 100ms
                0.0,    // No limit on keep age
                0,      // No limit on keep samples
            )
        };

        if result != 0 {
            if result == -29 {
                // DCGM_ST_REQUIRES_ROOT
                return Err(DcgmError::RequiresRoot(
                    "Profiling metrics require root access. Try running with sudo".to_string(),
                ));
            } else {
                return Err(DcgmError::ApiError(
                    result,
                    "dcgmWatchFields failed for profiling metrics".to_string(),
                ));
            }
        }

        self.prof_watched = true;
        Ok(())
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

    pub fn get_handle(&self) -> u64 {
        self.handle
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
}

impl Drop for DcgmHandle {
    fn drop(&mut self) {
        if self.handle != 0 {
            // Try to clean up
            unsafe {
                // First try to stop watching any fields if we created field groups
                if self.power_group_id != 0 || self.prof_group_id != 0 {
                    if let Ok(dcgm_unwatch_fields) =
                        self.lib
                            .get::<unsafe extern "C" fn(u64, u64, u64) -> i32>(b"dcgmUnwatchFields")
                    {
                        if self.power_group_id != 0 {
                            let _ =
                                dcgm_unwatch_fields(self.handle, 0x7fffffff, self.power_group_id);
                        }
                        if self.prof_group_id != 0 {
                            let _ =
                                dcgm_unwatch_fields(self.handle, 0x7fffffff, self.prof_group_id);
                        }
                    }
                }

                // Then destroy field groups
                if let Ok(dcgm_field_group_destroy) = self
                    .lib
                    .get::<unsafe extern "C" fn(u64, u64) -> i32>(b"dcgmFieldGroupDestroy")
                {
                    if self.power_group_id != 0 {
                        let _ = dcgm_field_group_destroy(self.handle, self.power_group_id);
                    }
                    if self.prof_group_id != 0 {
                        let _ = dcgm_field_group_destroy(self.handle, self.prof_group_id);
                    }
                }

                // Then stop embedded mode if we're in it
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
