// Field identifiers that we're interested in
pub const DCGM_FI_DEV_POWER_USAGE: u16 = 155;
pub const DCGM_FI_PROF_SM_ACTIVE: u16 = 1002;

// Field types
pub const DCGM_FT_DOUBLE: i8 = b'd' as i8;
pub const DCGM_FT_INT64: i8 = b'i' as i8;

// Entity group types
pub const DCGM_FE_GPU: u32 = 1;

// Constants for dcgmGetLatestValues
pub const DCGM_INT32_BLANK: i32 = 0x7ffffff0;
pub const DCGM_INT64_BLANK: i64 = 0x7ffffffffffffff0;
pub const DCGM_FP64_BLANK: f64 = 140737488355328.0;

// Flags for dcgmEntitiesGetLatestValues
pub const DCGM_FV_FLAG_LIVE_DATA: u32 = 0x00000001;

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
