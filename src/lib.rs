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
}

pub type Result<T> = std::result::Result<T, DcgmError>;

pub struct DcgmHandle {
    handle: u64,
    lib: Library,
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

        Ok(DcgmHandle { handle, lib })
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

        Ok(DcgmHandle { handle, lib })
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
                if let Ok(dcgm_stop_embedded) = self
                    .lib
                    .get::<unsafe extern "C" fn(u64) -> i32>(b"dcgmStopEmbedded")
                {
                    let _ = dcgm_stop_embedded(self.handle);
                }

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
