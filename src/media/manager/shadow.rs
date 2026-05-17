use std::sync::{Arc, Mutex};

use crate::foundation::error::TguiError;

use super::super::types::TextureFrame;

const MAX_CANVAS_SHADOW_CACHE_ENTRIES: usize = 16;
const MAX_WIDGET_SHADOW_CACHE_ENTRIES: usize = 24;

pub(super) struct CanvasShadowEntry {
    pub(super) cache_key: u64,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) texture: Arc<TextureFrame>,
    pub(super) last_used: u64,
}

pub(super) struct WidgetShadowEntry {
    pub(super) cache_key: u64,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) texture: Arc<TextureFrame>,
    pub(super) last_used: u64,
}

pub(super) fn canvas_shadow_texture<F>(
    cache: &Mutex<Vec<CanvasShadowEntry>>,
    cache_key: u64,
    width: u32,
    height: u32,
    render: F,
) -> Result<Option<Arc<TextureFrame>>, TguiError>
where
    F: FnOnce() -> Result<TextureFrame, TguiError>,
{
    shadow_texture(
        cache,
        MAX_CANVAS_SHADOW_CACHE_ENTRIES,
        cache_key,
        width,
        height,
        render,
    )
}

pub(super) fn widget_shadow_texture<F>(
    cache: &Mutex<Vec<WidgetShadowEntry>>,
    cache_key: u64,
    width: u32,
    height: u32,
    render: F,
) -> Result<Option<Arc<TextureFrame>>, TguiError>
where
    F: FnOnce() -> Result<TextureFrame, TguiError>,
{
    shadow_texture(
        cache,
        MAX_WIDGET_SHADOW_CACHE_ENTRIES,
        cache_key,
        width,
        height,
        render,
    )
}

trait ShadowCacheEntry {
    fn cache_key(&self) -> u64;
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn texture(&self) -> Arc<TextureFrame>;
    fn last_used(&self) -> u64;
    fn set_last_used(&mut self, value: u64);
    fn new(
        cache_key: u64,
        width: u32,
        height: u32,
        texture: Arc<TextureFrame>,
        last_used: u64,
    ) -> Self;
}

impl ShadowCacheEntry for CanvasShadowEntry {
    fn cache_key(&self) -> u64 {
        self.cache_key
    }
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    fn texture(&self) -> Arc<TextureFrame> {
        self.texture.clone()
    }
    fn last_used(&self) -> u64 {
        self.last_used
    }
    fn set_last_used(&mut self, value: u64) {
        self.last_used = value;
    }
    fn new(
        cache_key: u64,
        width: u32,
        height: u32,
        texture: Arc<TextureFrame>,
        last_used: u64,
    ) -> Self {
        Self {
            cache_key,
            width,
            height,
            texture,
            last_used,
        }
    }
}

impl ShadowCacheEntry for WidgetShadowEntry {
    fn cache_key(&self) -> u64 {
        self.cache_key
    }
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    fn texture(&self) -> Arc<TextureFrame> {
        self.texture.clone()
    }
    fn last_used(&self) -> u64 {
        self.last_used
    }
    fn set_last_used(&mut self, value: u64) {
        self.last_used = value;
    }
    fn new(
        cache_key: u64,
        width: u32,
        height: u32,
        texture: Arc<TextureFrame>,
        last_used: u64,
    ) -> Self {
        Self {
            cache_key,
            width,
            height,
            texture,
            last_used,
        }
    }
}

fn shadow_texture<T, F>(
    cache: &Mutex<Vec<T>>,
    max_entries: usize,
    cache_key: u64,
    width: u32,
    height: u32,
    render: F,
) -> Result<Option<Arc<TextureFrame>>, TguiError>
where
    T: ShadowCacheEntry,
    F: FnOnce() -> Result<TextureFrame, TguiError>,
{
    if width == 0 || height == 0 {
        return Ok(None);
    }

    let mut cache = cache.lock().expect("shadow cache lock poisoned");
    if let Some(entry) = cache.iter_mut().find(|entry| {
        entry.cache_key() == cache_key && entry.width() == width && entry.height() == height
    }) {
        entry.set_last_used(entry.last_used().saturating_add(1));
        return Ok(Some(entry.texture()));
    }

    let texture = Arc::new(render()?);
    let next_tick = cache
        .iter()
        .map(|entry| entry.last_used())
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    cache.push(T::new(cache_key, width, height, texture.clone(), next_tick));
    while cache.len() > max_entries {
        if let Some((oldest_index, _)) = cache
            .iter()
            .enumerate()
            .min_by_key(|(_, entry)| entry.last_used())
        {
            cache.remove(oldest_index);
        } else {
            break;
        }
    }

    Ok(Some(texture))
}
