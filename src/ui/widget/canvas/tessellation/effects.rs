use super::super::*;

#[derive(Clone, Copy)]
pub(in super::super) struct ResolvedCanvasEffects {
    pub blur_radius: f32,
    pub color_filter: Option<CanvasColorFilter>,
    pub inner_shadow: Option<CanvasInnerShadow>,
}

pub(in super::super) fn resolve_canvas_effects(effects: &[CanvasEffect]) -> ResolvedCanvasEffects {
    let mut blur_radius: f32 = 0.0;
    let mut color_filter = None;
    let mut inner_shadow = None;
    for effect in effects {
        match effect {
            CanvasEffect::Blur(radius) => {
                blur_radius = blur_radius.max(radius.get().max(0.0));
            }
            CanvasEffect::ColorFilter(filter) => {
                color_filter = Some(*filter);
            }
            CanvasEffect::InnerShadow(shadow) => {
                inner_shadow = Some(*shadow);
            }
        }
    }
    ResolvedCanvasEffects {
        blur_radius,
        color_filter,
        inner_shadow,
    }
}
