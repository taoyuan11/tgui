use super::*;

#[derive(Clone)]
pub struct CanvasTextHitRegion {
    pub hit: CanvasTextHit,
    pub quad: [Point; 4],
}

#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct RenderedWidgetScene {
    pub primitives: ScenePrimitives,
    pub scroll_regions: Vec<ScrollRegion>,
    #[allow(dead_code)]
    pub ime_cursor_area: Option<Rect>,
}
