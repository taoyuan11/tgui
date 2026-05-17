use super::*;

pub(crate) fn normalized_source_rect(
    source_rect: Option<Rect>,
    intrinsic_size: IntrinsicSize,
) -> Option<Rect> {
    let mut rect = source_rect?;
    if intrinsic_size.width <= 0.0 || intrinsic_size.height <= 0.0 {
        return None;
    }

    let min_x = rect.x.get().clamp(0.0, intrinsic_size.width);
    let min_y = rect.y.get().clamp(0.0, intrinsic_size.height);
    let max_x = (rect.x + rect.width)
        .get()
        .clamp(min_x, intrinsic_size.width);
    let max_y = (rect.y + rect.height)
        .get()
        .clamp(min_y, intrinsic_size.height);
    rect.x = Dp::new(min_x);
    rect.y = Dp::new(min_y);
    rect.width = Dp::new((max_x - min_x).max(0.0));
    rect.height = Dp::new((max_y - min_y).max(0.0));
    (!rect.is_empty()).then_some(rect)
}

pub(crate) fn intrinsic_size_from_rect(rect: Rect) -> IntrinsicSize {
    IntrinsicSize {
        width: rect.width.get().max(0.0),
        height: rect.height.get().max(0.0),
    }
}

pub(crate) fn source_rect_to_uv_rect(
    source_rect: Rect,
    intrinsic_size: IntrinsicSize,
) -> Option<Rect> {
    if intrinsic_size.width <= 0.0 || intrinsic_size.height <= 0.0 {
        return None;
    }

    Some(Rect::new(
        source_rect.x.get() / intrinsic_size.width,
        source_rect.y.get() / intrinsic_size.height,
        source_rect.width.get() / intrinsic_size.width,
        source_rect.height.get() / intrinsic_size.height,
    ))
}

pub(crate) fn raster_request_for_image(
    intrinsic_size: IntrinsicSize,
    source_rect: Option<Rect>,
    target_frame: Rect,
    units: UnitContext,
) -> Option<RasterRequest> {
    let mut request = RasterRequest::from_frame(target_frame, units.scale_factor())?;
    if let Some(source_rect) = source_rect {
        if source_rect.width > 0.0 && source_rect.height > 0.0 {
            let width_ratio = intrinsic_size.width / source_rect.width.get().max(f32::EPSILON);
            let height_ratio = intrinsic_size.height / source_rect.height.get().max(f32::EPSILON);
            let width = (request.width() as f32 * width_ratio).ceil().max(1.0) as u32;
            let height = (request.height() as f32 * height_ratio).ceil().max(1.0) as u32;
            request = RasterRequest::new_clamped(width, height);
        }
    }
    Some(request)
}
