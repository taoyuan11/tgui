use bytemuck::{Pod, Zeroable};
use cosmic_text::{FontSystem, SwashCache};

pub(super) struct TextSystem {
    pub(super) font_system: FontSystem,
    pub(super) swash_cache: SwashCache,
}

#[derive(Clone)]
pub(super) struct OffscreenTarget {
    pub(super) single_texture: wgpu::Texture,
    pub(super) single_view: wgpu::TextureView,
    pub(super) _msaa_texture: Option<wgpu::Texture>,
    pub(super) msaa_view: Option<wgpu::TextureView>,
}

pub(super) struct TextCacheEntry {
    pub(super) bind_group: wgpu::BindGroup,
    pub(super) _texture: wgpu::Texture,
}

pub(super) struct TextureCacheEntry {
    pub(super) bind_group: wgpu::BindGroup,
    pub(super) _texture: wgpu::Texture,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) struct TextCacheKey {
    pub(super) content: String,
    pub(super) font_family: Option<String>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) color: [u8; 4],
    pub(super) force_color: bool,
    pub(super) font_size_bits: u32,
    pub(super) line_height_bits: u32,
    pub(super) letter_spacing_bits: u32,
    pub(super) font_weight: u16,
    pub(super) wrap_mode: u8,
    pub(super) overflow_mode: u8,
    pub(super) horizontal_align: u8,
    pub(super) vertical_align: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct BlurUniform {
    pub(super) direction: [f32; 2],
    pub(super) texel_size: [f32; 2],
    pub(super) radius: f32,
    pub(super) _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct CompositeUniform {
    pub(super) data0: [f32; 4],
    pub(super) data1: [f32; 4],
    pub(super) data2: [f32; 4],
    pub(super) data3: [f32; 4],
    pub(super) data4: [f32; 4],
}
