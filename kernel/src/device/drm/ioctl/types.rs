// SPDX-License-Identifier: MPL-2.0

use aster_drm::{DRM_FORMAT_MAX_PLANES, DRM_PROP_NAME_LEN, DrmDisplayFormat, DrmModeModeInfo};
use int_to_c_enum::TryFromInt;

#[repr(C)]
#[padding_struct]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmVersion {
    pub version_major: i32,
    pub version_minor: i32,
    pub version_patchlevel: i32,

    pub name_len: usize,
    pub name: usize,
    pub date_len: usize,
    pub date: usize,
    pub desc_len: usize,
    pub desc: usize,
}

#[repr(u64)]
#[derive(Debug, TryFromInt)]
pub enum DrmGetCapability {
    DumbBuffer = 0x1,
    VblankHighCrtc = 0x2,
    DumbPreferredDepth = 0x3,
    DumbPreferShadow = 0x4,
    Prime = 0x5,
    TimestampMonotonic = 0x6,
    AsyncPageFlip = 0x7,
    CursorWidth = 0x8,
    CursorHeight = 0x9,
    Addfb2Modifiers = 0x10,
    PageFlipTarget = 0x11,
    CrtcInVblankEvent = 0x12,
    SyncObj = 0x13,
    SyncObjTimeline = 0x14,
    AtomicAsyncPageFlip = 0x15,
}

bitflags::bitflags! {
    pub struct DrmPrimeValue: u64 {
        const IMPORT = 0x1;
        const EXPORT = 0x2;
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmGetCap {
    pub capability: u64,
    pub value: u64,
}

#[repr(u64)]
#[derive(Debug, TryFromInt)]
pub enum DrmSetCapability {
    Stereo3D = 0x1,
    UniversalPlane = 0x2,
    Atomic = 0x3,
    AspectRatio = 0x4,
    WritebackConnectors = 0x5,
    CursorPlaneHostport = 0x6,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmSetClientCap {
    pub capability: u64,
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmModeGetResources {
    pub fb_id_ptr: u64,
    pub crtc_id_ptr: u64,
    pub connector_id_ptr: u64,
    pub encoder_id_ptr: u64,

    pub count_fbs: u32,
    pub count_crtcs: u32,
    pub count_connectors: u32,
    pub count_encoders: u32,

    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmModeCrtc {
    pub set_connectors_ptr: u64,
    pub count_connectors: u32,

    pub crtc_id: u32,
    pub fb_id: u32,

    pub x: u32,
    pub y: u32,

    pub gamma_size: u32,
    pub mode_valid: u32,
    pub mode: DrmModeModeInfo,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmModeGetEncoder {
    pub encoder_id: u32,
    pub encoder_type: u32,

    pub crtc_id: u32,

    pub possible_crtcs: u32,
    pub possible_clones: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmModeGetConnector {
    pub encoders_ptr: u64,
    pub modes_ptr: u64,
    pub props_ptr: u64,
    pub prop_values_ptr: u64,

    pub count_modes: u32,
    pub count_props: u32,
    pub count_encoders: u32,

    pub encoder_id: u32,
    pub connector_id: u32,

    pub connector_type: u32,
    pub connector_type_id: u32,

    pub connection: u32,

    pub mm_width: u32,
    pub mm_height: u32,
    pub subpixel: u32,

    pub pad: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmModeGetProperty {
    pub values_ptr: u64,
    pub enum_blob_ptr: u64,

    pub prop_id: u32,
    pub flags: u32,
    pub name: [u8; DRM_PROP_NAME_LEN],

    pub count_values: u32,
    pub count_enum_blobs: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmModeGetBlob {
    pub blob_id: u32,
    pub length: u32,
    pub data: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmModeFbCmd {
    pub fb_id: u32,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bpp: u32,
    pub depth: u32,
    pub handle: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmModeCreateDumb {
    pub height: u32,
    pub width: u32,
    pub bpp: u32,
    pub flags: u32,
    pub handle: u32,
    pub pitch: u32,
    pub size: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmModeMapDumb {
    pub handle: u32,
    pub pad: u32,
    pub offset: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmModeDestroyDumb {
    pub handle: u32,
}

#[repr(C)]
#[padding_struct]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmModeGetPlaneRes {
    pub plane_id_ptr: u64,
    pub count_planes: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmModeGetPlane {
    pub plane_id: u32,
    pub crtc_id: u32,
    pub fb_id: u32,
    pub possible_crtcs: u32,
    pub gamma_size: u32,
    pub count_format_types: u32,
    pub format_type_ptr: u64,
}

#[repr(C)]
#[padding_struct]
#[derive(Debug, Clone, Copy, Pod, Default)]
pub struct DrmModeFbCmd2 {
    pub fb_id: u32,
    pub width: u32,
    pub height: u32,
    pub pixel_format: u32,
    pub flags: u32,
    pub handles: [u32; DRM_FORMAT_MAX_PLANES],
    pub pitches: [u32; DRM_FORMAT_MAX_PLANES],
    pub offsets: [u32; DRM_FORMAT_MAX_PLANES],
    pub modifier: [u64; DRM_FORMAT_MAX_PLANES],
}

impl From<DrmModeFbCmd> for DrmModeFbCmd2 {
    fn from(fb_cmd: DrmModeFbCmd) -> Self {
        let pixel_format = match (fb_cmd.bpp, fb_cmd.depth) {
            (8, 8) => DrmDisplayFormat::C8 as u32,
            (16, 15) => DrmDisplayFormat::XRGB1555 as u32,
            (16, 16) => DrmDisplayFormat::RGB565 as u32,
            (24, 24) => DrmDisplayFormat::RGB888 as u32,
            (32, 24) => DrmDisplayFormat::XRGB8888 as u32,
            (32, 30) => DrmDisplayFormat::XRGB2101010 as u32,
            (32, 32) => DrmDisplayFormat::ARGB8888 as u32,
            _ => DrmDisplayFormat::Unknown as u32,
        };

        let mut handles = [0u32; 4];
        let mut pitches = [0u32; 4];
        let offsets = [0u32; 4];
        let modifier = [0u64; 4];

        handles[0] = fb_cmd.handle;
        pitches[0] = fb_cmd.pitch;

        Self {
            fb_id: fb_cmd.fb_id,
            width: fb_cmd.width,
            height: fb_cmd.height,
            pixel_format,
            flags: 0,
            handles,
            pitches,
            offsets,
            modifier,
            ..Default::default()
        }
    }
}

#[repr(C)]
#[padding_struct]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmModeObjectGetProps {
    pub props_ptr: u64,
    pub prop_values_ptr: u64,
    pub count_props: u32,
    pub obj_id: u32,
    pub obj_type: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmModeCreateBlob {
    pub data: u64,
    pub length: u32,
    pub blob_id: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmModeDestroyBlob {
    pub blob_id: u32,
}
