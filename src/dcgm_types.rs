// Field identifiers for metrics we're interested in
// GPU Utilization & Saturation Metrics
pub const DCGM_FI_DEV_GPU_UTIL: u16 = 203;

// FLOPs and Computational Efficiency Metrics
// These might require sudo access

// Memory Metrics
pub const DCGM_FI_DEV_FB_TOTAL: u16 = 250;
pub const DCGM_FI_DEV_FB_FREE: u16 = 251;
pub const DCGM_FI_DEV_FB_USED: u16 = 252;

// Power & Thermal Metrics
pub const DCGM_FI_DEV_POWER_USAGE: u16 = 155;
pub const DCGM_FI_DEV_TOTAL_ENERGY_CONSUMPTION: u16 = 156;
pub const DCGM_FI_DEV_GPU_TEMP: u16 = 150;
pub const DCGM_FI_DEV_ENFORCED_POWER_LIMIT: u16 = 164;
pub const DCGM_FI_DEV_GPU_MAX_OP_TEMP: u16 = 152;
pub const DCGM_FI_DEV_POWER_VIOLATION: u16 = 240;
pub const DCGM_FI_DEV_THERMAL_VIOLATION: u16 = 241;
pub const DCGM_FI_DEV_CLOCKS_EVENT_REASONS: u16 = 112;

pub const DCGM_FI_PROF_SM_ACTIVE: u16 = 1002;

// Device Metadata
pub const DCGM_FI_DEV_NAME: u16 = 50;

// Clock information
pub const DCGM_FI_DEV_SM_CLOCK: u16 = 100;
pub const DCGM_FI_DEV_MEM_CLOCK: u16 = 101;

// Field types
pub const DCGM_FT_DOUBLE: i8 = b'd' as i8;
pub const DCGM_FT_INT64: i8 = b'i' as i8;
pub const DCGM_FT_STRING: i8 = b's' as i8;

// Entity group types
pub const DCGM_FE_GPU: u32 = 1;

// Constants for dcgmGetLatestValues
pub const DCGM_INT32_BLANK: i32 = 0x7ffffff0;
pub const DCGM_INT64_BLANK: i64 = 0x7ffffffffffffff0;
pub const DCGM_FP64_BLANK: f64 = 140737488355328.0;

// Flags for dcgmEntitiesGetLatestValues
pub const DCGM_FV_FLAG_LIVE_DATA: u32 = 0x00000001;

// Clock events reason values
pub const DCGM_CLOCKS_EVENT_REASON_GPU_IDLE: u64 = 0x0000000000000001;
pub const DCGM_CLOCKS_EVENT_REASON_CLOCKS_SETTING: u64 = 0x0000000000000002;
pub const DCGM_CLOCKS_EVENT_REASON_SW_POWER_CAP: u64 = 0x0000000000000004;
pub const DCGM_CLOCKS_EVENT_REASON_HW_SLOWDOWN: u64 = 0x0000000000000008;
pub const DCGM_CLOCKS_EVENT_REASON_SYNC_BOOST: u64 = 0x0000000000000010;
pub const DCGM_CLOCKS_EVENT_REASON_SW_THERMAL: u64 = 0x0000000000000020;
pub const DCGM_CLOCKS_EVENT_REASON_HW_THERMAL: u64 = 0x0000000000000040;
pub const DCGM_CLOCKS_EVENT_REASON_HW_POWER_BRAKE: u64 = 0x0000000000000080;
pub const DCGM_CLOCKS_EVENT_REASON_DISPLAY_CLOCKS: u64 = 0x0000000000000100;

// Used to check if a value is blank
#[inline]
pub fn is_int64_blank(val: i64) -> bool {
    val >= DCGM_INT64_BLANK
}

#[inline]
pub fn is_fp64_blank(val: f64) -> bool {
    val >= DCGM_FP64_BLANK
}

#[repr(C)]
pub struct DcgmFieldValue {
    pub version: u32,
    pub entity_group_id: u32,
    pub entity_id: u32,
    pub field_id: u16,
    pub field_type: u16,
    pub status: i32,
    pub unused: u32,
    pub timestamp: i64,
    pub value: DcgmFieldValueUnion,
}

#[repr(C)]
pub union DcgmFieldValueUnion {
    pub i64: i64,
    pub dbl: f64,
    pub str: [i8; 256],   // DCGM_MAX_STR_LENGTH
    pub blob: [i8; 4096], // DCGM_MAX_BLOB_LENGTH
}

impl Default for DcgmFieldValue {
    fn default() -> Self {
        DcgmFieldValue {
            version: 2, // dcgmFieldValue_version2
            entity_group_id: 0,
            entity_id: 0,
            field_id: 0,
            field_type: 0,
            status: 0,
            unused: 0,
            timestamp: 0,
            value: unsafe { std::mem::zeroed() },
        }
    }
}
