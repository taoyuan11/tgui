use super::*;

pub(crate) fn item_text_hits(
    item: &CanvasItem,
    font_manager: &FontManager,
    origin: Point,
    units: UnitContext,
) -> Arc<[CanvasTextHitEntry]> {
    let CanvasItem::Text(text) = item else {
        return Arc::from([]);
    };
    let content = text.content.plain_text();
    if content.is_empty() {
        return Arc::from([]);
    }

    let line_height = text
        .text_style
        .line_height
        .unwrap_or(Sp::new(text.text_style.font_size.get() * 1.2))
        .get();
    let request = TextFontRequest {
        preferred_font: text.text_style.font_family.as_deref(),
        weight: text.text_style.font_weight,
    };
    let max_width = match text.paragraph_style.wrap {
        CanvasTextWrap::None => None,
        _ => Some(text.frame.width.get().max(0.0)),
    };
    let layout = canvas_text_layout(
        font_manager,
        text,
        &content,
        request,
        line_height,
        max_width,
        units,
    );
    let content_frame = canvas_text_content_frame(text, &layout, origin);
    let mut hits = Vec::new();

    for line_index in 0..layout.line_count() {
        let line_start = layout.line_start(line_index).min(content.len());
        let line_end = layout.line_end(line_index).min(content.len());
        if line_start > line_end {
            continue;
        }
        let line_top = content_frame.y + layout.line_top(line_index);
        let line_height_value = Dp::new(layout.line_height(line_index).max(line_height));
        let line_width = Dp::new(layout.line_width(line_index).max(0.0));

        let mut boundaries = Vec::new();
        let mut cursor = line_start;
        boundaries.push((cursor, layout.x_for_index(cursor)));
        while cursor < line_end {
            let next = next_grapheme_boundary(&content, cursor, line_end);
            boundaries.push((next, layout.x_for_index(next)));
            cursor = next;
        }

        for pair in boundaries.windows(2) {
            let (start, start_x) = pair[0];
            let (end, end_x) = pair[1];
            let width = (end_x - start_x).max(0.0);
            let rect = Rect::new(
                content_frame.x + start_x,
                line_top,
                width.max(1.0),
                line_height_value,
            );
            let quad = if text.style.transform == CanvasTransform2D::IDENTITY {
                rect_to_quad(rect)
            } else {
                transform_rect_quad(rect, text.style.transform, origin)
            };
            hits.push(CanvasTextHitEntry {
                hit: CanvasTextHit {
                    utf8_start: start,
                    utf8_end: end,
                    line_index,
                    line_start,
                    line_end,
                    line_top: Dp::new(layout.line_top(line_index)),
                    line_height: line_height_value,
                    line_width,
                    cluster_bounds: Rect::new(
                        start_x,
                        layout.line_top(line_index),
                        width.max(1.0),
                        line_height_value,
                    ),
                },
                quad,
            });
        }
    }

    Arc::from(hits)
}

fn canvas_text_layout(
    font_manager: &FontManager,
    text: &CanvasText,
    content: &str,
    request: TextFontRequest<'_>,
    line_height: f32,
    max_width: Option<f32>,
    units: UnitContext,
) -> crate::text::font::TextLayoutInfo {
    let font_size = units.resolve_sp(text.text_style.font_size);
    let letter_spacing = units.resolve_sp(text.text_style.letter_spacing);
    match max_width {
        Some(width) => font_manager.measure_text_layout_wrapped(
            content,
            request,
            font_size,
            line_height,
            letter_spacing,
            width,
        ),
        None => font_manager.measure_text_layout(
            content,
            request,
            font_size,
            line_height,
            letter_spacing,
        ),
    }
}

fn canvas_text_content_frame(
    text: &CanvasText,
    layout: &crate::text::font::TextLayoutInfo,
    origin: Point,
) -> Rect {
    let frame = offset_rect(text.frame, origin);
    let width = layout.width.max(0.0).min(frame.width.get());
    let height = layout.height.max(0.0).min(frame.height.get());
    let offset_x = match text.paragraph_style.horizontal_align {
        CanvasTextHorizontalAlign::Start => 0.0,
        CanvasTextHorizontalAlign::Center => (frame.width.get() - width).max(0.0) * 0.5,
        CanvasTextHorizontalAlign::End => (frame.width.get() - width).max(0.0),
    };
    let offset_y = match text.paragraph_style.vertical_align {
        CanvasTextVerticalAlign::Start => 0.0,
        CanvasTextVerticalAlign::Center => (frame.height.get() - height).max(0.0) * 0.5,
        CanvasTextVerticalAlign::End => (frame.height.get() - height).max(0.0),
    };
    Rect::new(frame.x + offset_x, frame.y + offset_y, width, height)
}

pub(crate) fn rect_to_quad(rect: Rect) -> [Point; 4] {
    [
        Point::new(rect.x, rect.y),
        Point::new(rect.right(), rect.y),
        Point::new(rect.right(), rect.bottom()),
        Point::new(rect.x, rect.bottom()),
    ]
}

fn next_grapheme_boundary(text: &str, start: usize, limit: usize) -> usize {
    if start >= limit {
        return limit;
    }
    text[start..limit]
        .grapheme_indices(true)
        .nth(1)
        .map(|(offset, _)| start + offset)
        .unwrap_or(limit)
}
