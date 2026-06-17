use super::super::*;
use super::shadow::{rasterize_rounded_rect_shadow, rounded_rect_shadow_cache_key, shadow_padding};
use crate::media::TextureFrame;
use std::sync::{Arc, OnceLock};

fn transparent_media_texture() -> Arc<TextureFrame> {
    static TEXTURE: OnceLock<Arc<TextureFrame>> = OnceLock::new();
    Arc::clone(TEXTURE.get_or_init(|| Arc::new(TextureFrame::new(1, 1, vec![0, 0, 0, 0]))))
}

pub(crate) fn push_media_texture_or_placeholder<VM>(
    widget_id: WidgetId,
    source: &crate::media::MediaSource,
    fit: ContentFit,
    frame: Rect,
    content_frame: Rect,
    content_corner_radius: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    opacity: f32,
    loading_background: Color,
    context: &mut CollectContext<'_, '_>,
    computed: &mut ComputedScene<VM>,
    kind: &str,
) {
    let metadata = context.media.image_snapshot(source, None);
    let target_frame = resolve_media_rect(content_frame, metadata.intrinsic_size, fit);
    let raster_request = RasterRequest::from_frame(target_frame, context.units.scale_factor());
    let media_key =
        raster_request.map(|request| crate::media::MediaTextureKey::new(source.clone(), request));
    let media_layout = Some(crate::media::MediaTextureLayout::new(
        content_frame,
        fit,
        context.units.scale_factor(),
    ));
    let snapshot = if let Some(raster_request) = raster_request {
        context.media.image_snapshot(source, Some(raster_request))
    } else {
        metadata
    };

    if let Some(texture) = snapshot.texture.as_ref() {
        if media_key.is_some() {
            push_media_placeholder(
                frame,
                content_frame,
                content_corner_radius,
                clip_rect,
                clip_mask,
                0.0,
                context,
                &mut computed.scene,
                widget_id,
                kind,
                true,
                None,
                loading_background,
                true,
            );
        }
        computed.scene.push_texture(TexturePrimitive {
            texture: Arc::clone(texture),
            media_key,
            media_layout,
            frame: target_frame,
            quad: None,
            uv_rect: None,
            corner_radius: content_corner_radius,
            opacity: opacity.clamp(0.0, 1.0),
            clip_rect,
            clip_mask,
        });
        return;
    }

    if media_key.is_some() && snapshot.loading && snapshot.error.is_none() {
        push_media_placeholder(
            frame,
            content_frame,
            content_corner_radius,
            clip_rect,
            clip_mask,
            opacity,
            context,
            &mut computed.scene,
            widget_id,
            kind,
            snapshot.loading,
            snapshot.error.as_deref(),
            loading_background,
            snapshot.loading,
        );
        computed.scene.push_texture(TexturePrimitive {
            texture: transparent_media_texture(),
            media_key,
            media_layout,
            frame: target_frame,
            quad: None,
            uv_rect: None,
            corner_radius: content_corner_radius,
            opacity: opacity.clamp(0.0, 1.0),
            clip_rect,
            clip_mask,
        });
        return;
    }

    push_media_placeholder(
        frame,
        content_frame,
        content_corner_radius,
        clip_rect,
        clip_mask,
        opacity,
        context,
        &mut computed.scene,
        widget_id,
        kind,
        snapshot.loading,
        snapshot.error.as_deref(),
        loading_background,
        snapshot.loading,
    );
}

pub(crate) fn push_background_media_texture<VM>(
    source: &crate::media::MediaSource,
    fit: ContentFit,
    content_frame: Rect,
    content_corner_radius: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    context: &mut CollectContext<'_, '_>,
    computed: &mut ComputedScene<VM>,
) {
    let metadata = context.media.image_snapshot(source, None);
    let target_frame = resolve_media_rect(content_frame, metadata.intrinsic_size, fit);
    let raster_request = RasterRequest::from_frame(target_frame, context.units.scale_factor());
    let media_layout = Some(crate::media::MediaTextureLayout::new(
        content_frame,
        fit,
        context.units.scale_factor(),
    ));
    let snapshot = if let Some(raster_request) = raster_request {
        context.media.image_snapshot(source, Some(raster_request))
    } else {
        metadata
    };

    if let Some(texture) = snapshot.texture.as_ref() {
        computed.scene.push_texture(TexturePrimitive {
            texture: Arc::clone(texture),
            media_key: raster_request
                .map(|request| crate::media::MediaTextureKey::new(source.clone(), request)),
            media_layout,
            frame: target_frame,
            quad: None,
            uv_rect: None,
            corner_radius: content_corner_radius,
            opacity: 1.0,
            clip_rect,
            clip_mask,
        });
    } else if let Some(raster_request) = raster_request {
        if snapshot.loading && snapshot.error.is_none() {
            computed.scene.push_texture(TexturePrimitive {
                texture: transparent_media_texture(),
                media_key: Some(crate::media::MediaTextureKey::new(
                    source.clone(),
                    raster_request,
                )),
                media_layout,
                frame: target_frame,
                quad: None,
                uv_rect: None,
                corner_radius: content_corner_radius,
                opacity: 1.0,
                clip_rect,
                clip_mask,
            });
        }
    }
}

#[derive(Clone)]
pub(crate) struct RoundedRectShadowSpec {
    pub(crate) shadow: crate::theme::Shadow,
    pub(crate) opacity: f32,
    pub(crate) clip_rect: Option<Rect>,
    pub(crate) clip_mask: Option<ClipMask>,
}

pub(crate) fn rounded_rect_shadow_texture(
    frame: Rect,
    corner_radius: f32,
    spec: RoundedRectShadowSpec,
    media: &MediaManager,
    units: UnitContext,
) -> Option<TexturePrimitive> {
    if frame.width <= Dp::ZERO || frame.height <= Dp::ZERO {
        return None;
    }

    let spread = spec.shadow.spread.get();
    let expanded = Rect::new(
        frame.x - Dp::new(spread),
        frame.y - Dp::new(spread),
        (frame.width.get() + spread * 2.0).max(0.0),
        (frame.height.get() + spread * 2.0).max(0.0),
    );
    if expanded.width <= Dp::ZERO || expanded.height <= Dp::ZERO {
        return None;
    }

    let blur = spec.shadow.blur.get().max(0.0);
    let padding = shadow_padding(blur);
    let min_x = expanded.x.get() + spec.shadow.offset_x.get().min(0.0) - padding;
    let min_y = expanded.y.get() + spec.shadow.offset_y.get().min(0.0) - padding;
    let max_x = expanded.right().get() + spec.shadow.offset_x.get().max(0.0) + padding;
    let max_y = expanded.bottom().get() + spec.shadow.offset_y.get().max(0.0) + padding;
    let texture_frame = Rect::new(
        min_x,
        min_y,
        (max_x - min_x).max(1.0),
        (max_y - min_y).max(1.0),
    );
    let width = units
        .logical_to_physical(texture_frame.width.get())
        .ceil()
        .max(1.0) as u32;
    let height = units
        .logical_to_physical(texture_frame.height.get())
        .ceil()
        .max(1.0) as u32;
    let radius = (corner_radius + spread)
        .max(0.0)
        .min(expanded.width.min(expanded.height).get() * 0.5);
    let effective_color = spec
        .shadow
        .color
        .with_alpha_factor(spec.opacity.clamp(0.0, 1.0));
    if effective_color.a == 0 {
        return None;
    }

    let shadow = spec.shadow.clone();
    let cache_key = rounded_rect_shadow_cache_key(
        expanded,
        radius,
        shadow.clone(),
        spec.opacity,
        units.scale_factor(),
    );
    let texture = media
        .widget_shadow_texture(cache_key, width, height, || {
            rasterize_rounded_rect_shadow(
                expanded,
                radius,
                shadow,
                spec.opacity,
                min_x,
                min_y,
                width,
                height,
                units.scale_factor(),
            )
        })
        .ok()??;

    Some(TexturePrimitive {
        texture,
        media_key: None,
        media_layout: None,
        frame: texture_frame,
        quad: None,
        uv_rect: None,
        corner_radius: 0.0,
        opacity: 1.0,
        clip_rect: spec.clip_rect,
        clip_mask: spec.clip_mask,
    })
}

#[cfg(feature = "video")]
pub(crate) fn push_video_texture_or_placeholder<VM>(
    widget_id: WidgetId,
    video: &PublicVideoSurface,
    frame: Rect,
    content_frame: Rect,
    content_corner_radius: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    opacity: f32,
    loading_background: Color,
    context: &mut CollectContext<'_, '_>,
    computed: &mut ComputedScene<VM>,
) {
    let snapshot = video.controller.surface_metadata();
    let target_frame = resolve_media_rect(content_frame, snapshot.intrinsic_size, video.fit);
    let target_raster = RasterRequest::from_frame(target_frame, context.units.scale_factor());
    video.controller.set_target_raster(target_raster);
    let current_frame = video.controller.current_frame();
    let use_surface_background =
        snapshot.loading || (current_frame.is_none() && snapshot.error.is_none());

    if current_frame.is_some() {
        computed.scene.push_video_texture(VideoTexturePrimitive {
            controller: video.controller.clone(),
            frame: target_frame,
            quad: None,
            uv_rect: None,
            corner_radius: content_corner_radius,
            opacity: 1.0,
            clip_rect,
            clip_mask,
        });
        return;
    }

    push_media_placeholder(
        frame,
        content_frame,
        content_corner_radius,
        clip_rect,
        clip_mask,
        opacity,
        context,
        &mut computed.scene,
        widget_id,
        "video",
        snapshot.loading,
        snapshot.error.as_deref(),
        loading_background,
        use_surface_background,
    );
}

pub(crate) fn push_media_placeholder(
    frame: Rect,
    content_frame: Rect,
    content_corner_radius: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    opacity: f32,
    context: &mut CollectContext<'_, '_>,
    scene: &mut ScenePrimitives,
    widget_id: WidgetId,
    kind: &str,
    loading: bool,
    error: Option<&str>,
    loading_background: Color,
    use_loading_background: bool,
) {
    let placeholder =
        media_loading_fill_color(loading, error, loading_background, use_loading_background)
            .with_alpha_factor(opacity);
    if content_frame.width > Dp::ZERO && content_frame.height > Dp::ZERO {
        scene.push_shape(RenderPrimitive {
            rect: content_frame,
            color: placeholder,
            corner_radius: content_corner_radius,
            stroke_width: 0.0,
            clip_rect,
            clip_mask,
        });
    }

    let label = media_placeholder_label(kind, loading, error);
    let mut text = Text::new(label);
    text.font_size = Some((context.theme.typography.body_small.size - sp(1.0)).max(sp(12.0)));
    push_text_primitives(
        &text,
        frame,
        context.font_manager,
        context.theme,
        context.units,
        context.animations,
        context.now,
        scene,
        false,
        true,
        Insets::all(dp(12.0)),
        None,
        None,
        Color::hexa(0xE5E7EBFF),
        opacity,
        widget_id,
        clip_rect,
        clip_mask,
    );
}

pub(crate) fn media_loading_fill_color(
    loading: bool,
    error: Option<&str>,
    loading_background: Color,
    use_loading_background: bool,
) -> Color {
    if use_loading_background {
        loading_background
    } else {
        media_placeholder_color(loading, error)
    }
}
