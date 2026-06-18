use super::super::super::*;

pub(in super::super) fn canvas_item_kind_name(kind: CanvasItemKind) -> &'static str {
    match kind {
        CanvasItemKind::Path => "path",
        CanvasItemKind::Text => "text",
        CanvasItemKind::Image => "image",
        CanvasItemKind::Group => "group",
    }
}

pub(in super::super) fn canvas_fill_rule_name(fill_rule: CanvasFillRule) -> &'static str {
    match fill_rule {
        CanvasFillRule::NonZero => "non_zero",
        CanvasFillRule::EvenOdd => "even_odd",
    }
}

pub(in super::super) fn canvas_blend_mode_name(mode: CanvasBlendMode) -> &'static str {
    match mode {
        CanvasBlendMode::Normal => "normal",
        CanvasBlendMode::Multiply => "multiply",
        CanvasBlendMode::Screen => "screen",
        CanvasBlendMode::Overlay => "overlay",
        CanvasBlendMode::Darken => "darken",
        CanvasBlendMode::Lighten => "lighten",
        CanvasBlendMode::ColorDodge => "color_dodge",
        CanvasBlendMode::ColorBurn => "color_burn",
        CanvasBlendMode::HardLight => "hard_light",
        CanvasBlendMode::SoftLight => "soft_light",
        CanvasBlendMode::Difference => "difference",
        CanvasBlendMode::Exclusion => "exclusion",
        CanvasBlendMode::Plus => "plus",
    }
}

pub(super) fn canvas_stroke_cap_name(cap: CanvasStrokeCap) -> &'static str {
    match cap {
        CanvasStrokeCap::Butt => "butt",
        CanvasStrokeCap::Square => "square",
        CanvasStrokeCap::Round => "round",
    }
}

pub(super) fn canvas_stroke_join_name(join: CanvasStrokeJoin) -> &'static str {
    match join {
        CanvasStrokeJoin::Miter => "miter",
        CanvasStrokeJoin::Bevel => "bevel",
        CanvasStrokeJoin::Round => "round",
    }
}

pub(super) fn canvas_stroke_alignment_name(alignment: CanvasStrokeAlignment) -> &'static str {
    match alignment {
        CanvasStrokeAlignment::Center => "center",
        CanvasStrokeAlignment::Inside => "inside",
        CanvasStrokeAlignment::Outside => "outside",
    }
}

pub(in super::super) fn canvas_group_mode_name(mode: &CanvasGroupMode) -> &'static str {
    match mode {
        CanvasGroupMode::Clip => "clip",
        CanvasGroupMode::Mask => "mask",
    }
}

pub(crate) fn canvas_text_wrap_name(wrap: CanvasTextWrap) -> &'static str {
    match wrap {
        CanvasTextWrap::Word => "word",
        CanvasTextWrap::Glyph => "glyph",
        CanvasTextWrap::None => "none",
    }
}

pub(crate) fn canvas_text_horizontal_align_name(align: CanvasTextHorizontalAlign) -> &'static str {
    match align {
        CanvasTextHorizontalAlign::Start => "start",
        CanvasTextHorizontalAlign::Center => "center",
        CanvasTextHorizontalAlign::End => "end",
    }
}

pub(crate) fn canvas_text_vertical_align_name(align: CanvasTextVerticalAlign) -> &'static str {
    match align {
        CanvasTextVerticalAlign::Start => "start",
        CanvasTextVerticalAlign::Center => "center",
        CanvasTextVerticalAlign::End => "end",
    }
}

pub(crate) fn canvas_text_overflow_name(overflow: CanvasTextOverflow) -> &'static str {
    match overflow {
        CanvasTextOverflow::Clip => "clip",
        CanvasTextOverflow::Ellipsis => "ellipsis",
    }
}

pub(in super::super) fn content_fit_name(fit: ContentFit) -> &'static str {
    match fit {
        ContentFit::Contain => "contain",
        ContentFit::Cover => "cover",
        ContentFit::Fill => "fill",
    }
}
