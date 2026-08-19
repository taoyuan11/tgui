use crate::core::{
    Clip, Color, CornerRadii, Error, FontHandle, GlyphPageId, ImageHandle, Point, Rect, ResourceId,
    Result, Transform2D,
};
use std::fmt;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BlendMode {
    #[default]
    SourceOver,
    Copy,
    Multiply,
    Screen,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ImageSampling {
    Nearest,
    #[default]
    Linear,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Color,
}

impl GradientStop {
    pub fn new(offset: f32, color: Color) -> Result<Self> {
        let stop = Self { offset, color };
        if !offset.is_finite() || !(0.0..=1.0).contains(&offset) {
            return Err(Error::invalid_input(
                Some("gradient_stop.offset".to_owned()),
                "must be finite and between zero and one",
            ));
        }
        Ok(stop)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LinearGradient {
    pub start: Point,
    pub end: Point,
    pub stops: Arc<[GradientStop]>,
}

impl LinearGradient {
    pub fn new(
        start: Point,
        end: Point,
        stops: impl IntoIterator<Item = GradientStop>,
    ) -> Result<Self> {
        start.validate().map_err(Error::from)?;
        end.validate().map_err(Error::from)?;
        if start == end {
            return Err(Error::invalid_input(
                Some("linear_gradient".to_owned()),
                "start and end points must differ",
            ));
        }
        let stops = stops.into_iter().collect::<Vec<_>>();
        if stops.is_empty() {
            return Err(Error::invalid_input(
                Some("linear_gradient.stops".to_owned()),
                "at least one gradient stop is required",
            ));
        }
        let mut previous = 0.0;
        for (index, stop) in stops.iter().enumerate() {
            if !stop.offset.is_finite() || !(0.0..=1.0).contains(&stop.offset) {
                return Err(Error::invalid_input(
                    Some("linear_gradient.stops".to_owned()),
                    "stop offset is outside the unit interval",
                ));
            }
            if index != 0 && stop.offset < previous {
                return Err(Error::invalid_input(
                    Some("linear_gradient.stops".to_owned()),
                    "gradient stops must be sorted",
                ));
            }
            previous = stop.offset;
        }
        Ok(Self {
            start,
            end,
            stops: stops.into(),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Brush {
    Solid(Color),
    LinearGradient(LinearGradient),
}

impl From<Color> for Brush {
    fn from(value: Color) -> Self {
        Self::Solid(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub offset: Point,
    pub blur_radius: f32,
    pub spread: f32,
    pub color: Color,
}

impl Shadow {
    pub const fn new(offset: Point, blur_radius: f32, spread: f32, color: Color) -> Self {
        Self {
            offset,
            blur_radius,
            spread,
            color,
        }
    }

    fn validate(self) -> Result<()> {
        self.offset.validate().map_err(Error::from)?;
        if !self.blur_radius.is_finite() || self.blur_radius < 0.0 {
            return Err(Error::invalid_input(
                Some("shadow.blur_radius".to_owned()),
                "must be finite and non-negative",
            ));
        }
        if !self.spread.is_finite() {
            return Err(Error::invalid_input(
                Some("shadow.spread".to_owned()),
                "must be finite",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Paint {
    pub brush: Brush,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub shadow: Option<Shadow>,
}

impl Paint {
    pub fn solid(color: Color) -> Self {
        Self {
            brush: Brush::Solid(color),
            opacity: 1.0,
            blend_mode: BlendMode::SourceOver,
            shadow: None,
        }
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub fn with_blend_mode(mut self, mode: BlendMode) -> Self {
        self.blend_mode = mode;
        self
    }

    pub fn with_shadow(mut self, shadow: Shadow) -> Self {
        self.shadow = Some(shadow);
        self
    }

    fn validate(&self) -> Result<()> {
        match &self.brush {
            Brush::Solid(_) => {}
            Brush::LinearGradient(gradient) => {
                LinearGradient::new(gradient.start, gradient.end, gradient.stops.iter().cloned())?;
            }
        }
        validate_opacity("paint.opacity", self.opacity)?;
        if let Some(shadow) = self.shadow {
            shadow.validate()?;
        }
        Ok(())
    }
}

impl From<Color> for Paint {
    fn from(value: Color) -> Self {
        Self::solid(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeStyle {
    pub width: f32,
    pub miter_limit: f32,
}

impl StrokeStyle {
    pub const fn new(width: f32) -> Self {
        Self {
            width,
            miter_limit: 4.0,
        }
    }

    fn validate(self) -> Result<()> {
        if !self.width.is_finite() || self.width <= 0.0 || !self.miter_limit.is_finite() {
            return Err(Error::invalid_input(
                Some("stroke".to_owned()),
                "stroke width and miter limit must be finite and positive",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathSegment {
    MoveTo(Point),
    LineTo(Point),
    QuadraticTo {
        control: Point,
        to: Point,
    },
    CubicTo {
        control1: Point,
        control2: Point,
        to: Point,
    },
    Close,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    segments: Arc<[PathSegment]>,
    fill_rule: FillRule,
}

impl Path {
    pub fn new(
        segments: impl IntoIterator<Item = PathSegment>,
        fill_rule: FillRule,
    ) -> Result<Self> {
        let segments = segments.into_iter().collect::<Vec<_>>();
        if !matches!(segments.first(), Some(PathSegment::MoveTo(_))) {
            return Err(Error::invalid_input(
                Some("path.segments".to_owned()),
                "a path must begin with MoveTo",
            ));
        }
        for segment in segments.iter().copied() {
            match segment {
                PathSegment::MoveTo(point) | PathSegment::LineTo(point) => {
                    point.validate().map_err(Error::from)?;
                }
                PathSegment::QuadraticTo { control, to } => {
                    control.validate().map_err(Error::from)?;
                    to.validate().map_err(Error::from)?;
                }
                PathSegment::CubicTo {
                    control1,
                    control2,
                    to,
                } => {
                    control1.validate().map_err(Error::from)?;
                    control2.validate().map_err(Error::from)?;
                    to.validate().map_err(Error::from)?;
                }
                PathSegment::Close => {}
            }
        }
        Ok(Self {
            segments: segments.into(),
            fill_rule,
        })
    }

    pub fn segments(&self) -> &[PathSegment] {
        &self.segments
    }

    pub const fn fill_rule(&self) -> FillRule {
        self.fill_rule
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BackdropFilter {
    Blur { radius: f32 },
    Saturate { amount: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayerSpec {
    pub bounds: Rect,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub backdrop: Option<BackdropFilter>,
}

impl LayerSpec {
    pub const fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            opacity: 1.0,
            blend_mode: BlendMode::SourceOver,
            backdrop: None,
        }
    }

    pub const fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub const fn with_backdrop(mut self, filter: BackdropFilter) -> Self {
        self.backdrop = Some(filter);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    pub layout: ResourceId,
    pub font: FontHandle,
    pub glyph_page: Option<GlyphPageId>,
    pub bounds: Rect,
    pub color: Color,
    pub glyph_count: u32,
    pub content_revision: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum PaintCommand {
    Clear(Color),
    FillRect {
        rect: Rect,
        color: Color,
    },
    DrawRect {
        rect: Rect,
        paint: Paint,
    },
    DrawRoundedRect {
        rect: Rect,
        radii: CornerRadii,
        paint: Paint,
    },
    FillPath {
        path: Path,
        paint: Paint,
    },
    StrokePath {
        path: Path,
        style: StrokeStyle,
        paint: Paint,
    },
    PushClip(Clip),
    PopClip,
    PushTransform(Transform2D),
    PopTransform,
    BeginLayer(LayerSpec),
    EndLayer,
    DrawTextRun(TextRun),
    DrawImage {
        rect: Rect,
        image: ImageHandle,
        sampling: ImageSampling,
        opacity: f32,
    },
    DrawGlyphAtlas {
        rect: Rect,
        uv: Rect,
        page: GlyphPageId,
        color: Color,
    },
    NativeSurface {
        rect: Rect,
        surface: ResourceId,
        opaque: bool,
    },
    Marker(Arc<str>),
}

impl PaintCommand {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Clear(_) | Self::PopClip | Self::PopTransform | Self::EndLayer => Ok(()),
            Self::Marker(marker) => {
                if marker.contains('\n') || marker.contains('\r') {
                    return Err(Error::invalid_input(
                        Some("marker".to_owned()),
                        "line breaks are not stable",
                    ));
                }
                Ok(())
            }
            Self::FillRect { rect, .. } => rect.validate().map_err(Error::from),
            Self::DrawRect { rect, paint } => {
                rect.validate().map_err(Error::from)?;
                paint.validate()
            }
            Self::DrawRoundedRect { rect, radii, paint } => {
                rect.validate().map_err(Error::from)?;
                radii.validate().map_err(Error::from)?;
                paint.validate()
            }
            Self::FillPath { path, paint } => {
                path.validate_shallow()?;
                paint.validate()
            }
            Self::StrokePath { path, style, paint } => {
                path.validate_shallow()?;
                style.validate()?;
                paint.validate()
            }
            Self::PushClip(clip) => clip.validate().map_err(Error::from),
            Self::PushTransform(transform) => transform.validate().map_err(Error::from),
            Self::BeginLayer(layer) => {
                layer.bounds.validate().map_err(Error::from)?;
                validate_opacity("layer.opacity", layer.opacity)
            }
            Self::DrawTextRun(run) => {
                run.bounds.validate().map_err(Error::from)?;
                valid_handle("text_run.layout", run.layout.is_well_formed())?;
                valid_handle("text_run.font", run.font.is_well_formed())?;
                if let Some(page) = run.glyph_page {
                    valid_handle("text_run.glyph_page", page.is_well_formed())?;
                }
                Ok(())
            }
            Self::DrawImage {
                rect,
                image,
                opacity,
                ..
            } => {
                rect.validate().map_err(Error::from)?;
                valid_handle("image", image.is_well_formed())?;
                validate_opacity("image.opacity", *opacity)
            }
            Self::DrawGlyphAtlas { rect, uv, page, .. } => {
                rect.validate().map_err(Error::from)?;
                uv.validate().map_err(Error::from)?;
                valid_handle("glyph_page", page.is_well_formed())
            }
            Self::NativeSurface { rect, surface, .. } => {
                rect.validate().map_err(Error::from)?;
                valid_handle("native_surface", surface.is_well_formed())
            }
        }
    }

    pub fn stable_debug(&self) -> String {
        match self {
            Self::Clear(color) => format!("clear {}", color_text(*color)),
            Self::FillRect { rect, color } => {
                format!("fill_rect {} {}", rect_text(*rect), color_text(*color))
            }
            Self::DrawRect { rect, paint } => {
                format!("draw_rect {} {}", rect_text(*rect), paint_text(paint))
            }
            Self::DrawRoundedRect { rect, radii, paint } => format!(
                "draw_rounded_rect {} {} {}",
                rect_text(*rect),
                radii_text(*radii),
                paint_text(paint)
            ),
            Self::FillPath { path, paint } => format!(
                "fill_path {} {:?} {}",
                path.segments.len(),
                path.fill_rule,
                paint_text(paint)
            ),
            Self::StrokePath { path, style, paint } => format!(
                "stroke_path {} {:.6} {:.6} {}",
                path.segments.len(),
                style.width,
                style.miter_limit,
                paint_text(paint)
            ),
            Self::PushClip(clip) => match clip {
                Clip::Rect(rect) => format!("push_clip rect {}", rect_text(*rect)),
                Clip::RoundedRect { rect, radii } => format!(
                    "push_clip rounded_rect {} {}",
                    rect_text(*rect),
                    radii_text(*radii)
                ),
            },
            Self::PopClip => "pop_clip".to_owned(),
            Self::PushTransform(transform) => format!(
                "push_transform [{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}]",
                transform.m11,
                transform.m12,
                transform.m21,
                transform.m22,
                transform.tx,
                transform.ty
            ),
            Self::PopTransform => "pop_transform".to_owned(),
            Self::BeginLayer(layer) => format!(
                "begin_layer {} {:.6} {:?} {:?}",
                rect_text(layer.bounds),
                layer.opacity,
                layer.blend_mode,
                layer.backdrop
            ),
            Self::EndLayer => "end_layer".to_owned(),
            Self::DrawTextRun(run) => format!(
                "draw_text_run {}:{} font={}:{} page={:?} glyphs={} content={} {} {}",
                run.layout.slot(),
                run.layout.generation(),
                run.font.slot(),
                run.font.generation(),
                run.glyph_page,
                run.glyph_count,
                run.content_revision,
                rect_text(run.bounds),
                color_text(run.color)
            ),
            Self::DrawImage {
                rect,
                image,
                sampling,
                opacity,
            } => format!(
                "draw_image {}:{} {} {:?} {:.6}",
                image.slot(),
                image.generation(),
                rect_text(*rect),
                sampling,
                opacity
            ),
            Self::DrawGlyphAtlas {
                rect,
                uv,
                page,
                color,
            } => format!(
                "draw_glyph_atlas {}:{} {} {} {}",
                page.slot(),
                page.generation(),
                rect_text(*rect),
                rect_text(*uv),
                color_text(*color)
            ),
            Self::NativeSurface {
                rect,
                surface,
                opaque,
            } => format!(
                "native_surface {}:{} {} opaque={opaque}",
                surface.slot(),
                surface.generation(),
                rect_text(*rect)
            ),
            Self::Marker(marker) => format!("marker {marker:?}"),
        }
    }
}

impl Path {
    fn validate_shallow(&self) -> Result<()> {
        if self.segments.is_empty() {
            return Err(Error::compile("paint_ir", "path has no segments"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaintSnapshot {
    text: String,
    command_count: usize,
    fingerprint: u64,
}

impl PaintSnapshot {
    pub fn text(&self) -> &str {
        &self.text
    }
    pub const fn command_count(&self) -> usize {
        self.command_count
    }
    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
}

impl fmt::Display for PaintSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct StackDepths {
    clips: usize,
    transforms: usize,
    layers: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Canvas {
    commands: Vec<PaintCommand>,
    depths: StackDepths,
}

impl Canvas {
    pub const fn new() -> Self {
        Self {
            commands: Vec::new(),
            depths: StackDepths {
                clips: 0,
                transforms: 0,
                layers: 0,
            },
        }
    }
    pub fn commands(&self) -> &[PaintCommand] {
        &self.commands
    }
    pub fn record(&mut self, command: PaintCommand) -> Result<()> {
        command.validate()?;
        apply_stack(&mut self.depths, &command)?;
        self.commands.push(command);
        Ok(())
    }
    pub fn replay(&mut self, commands: &[PaintCommand]) -> Result<()> {
        let mut candidate = self.clone();
        for command in commands {
            candidate.record(command.clone())?;
        }
        *self = candidate;
        Ok(())
    }
    pub fn clear(&mut self, color: Color) -> Result<()> {
        self.record(PaintCommand::Clear(color))
    }
    pub fn fill_rect(&mut self, rect: Rect, color: Color) -> Result<()> {
        self.record(PaintCommand::FillRect { rect, color })
    }
    pub fn draw_rect(&mut self, rect: Rect, paint: Paint) -> Result<()> {
        self.record(PaintCommand::DrawRect { rect, paint })
    }
    pub fn draw_rounded_rect(
        &mut self,
        rect: Rect,
        radii: CornerRadii,
        paint: Paint,
    ) -> Result<()> {
        self.record(PaintCommand::DrawRoundedRect { rect, radii, paint })
    }
    pub fn push_clip(&mut self, clip: Clip) -> Result<()> {
        self.record(PaintCommand::PushClip(clip))
    }
    pub fn pop_clip(&mut self) -> Result<()> {
        self.record(PaintCommand::PopClip)
    }
    pub fn push_transform(&mut self, transform: Transform2D) -> Result<()> {
        self.record(PaintCommand::PushTransform(transform))
    }
    pub fn pop_transform(&mut self) -> Result<()> {
        self.record(PaintCommand::PopTransform)
    }
    pub fn begin_layer(&mut self, layer: LayerSpec) -> Result<()> {
        self.record(PaintCommand::BeginLayer(layer))
    }
    pub fn end_layer(&mut self) -> Result<()> {
        self.record(PaintCommand::EndLayer)
    }
    pub fn marker(&mut self, marker: impl Into<Arc<str>>) -> Result<()> {
        self.record(PaintCommand::Marker(marker.into()))
    }
    pub fn validate(&self) -> Result<()> {
        validate_commands(&self.commands)
    }
    pub fn snapshot(&self) -> Result<PaintSnapshot> {
        self.validate()?;
        Ok(snapshot_commands(&self.commands))
    }
    pub fn finish(self) -> Result<(Arc<[PaintCommand]>, PaintSnapshot)> {
        let snapshot = self.snapshot()?;
        Ok((self.commands.into(), snapshot))
    }
}

pub(crate) fn validate_commands(commands: &[PaintCommand]) -> Result<()> {
    let mut depths = StackDepths::default();
    for command in commands {
        command.validate()?;
        apply_stack(&mut depths, command)?;
    }
    if depths != StackDepths::default() {
        return Err(Error::compile(
            "paint_ir",
            format!(
                "unbalanced stacks (clip {}, transform {}, layer {})",
                depths.clips, depths.transforms, depths.layers
            ),
        ));
    }
    Ok(())
}

fn apply_stack(depths: &mut StackDepths, command: &PaintCommand) -> Result<()> {
    match command {
        PaintCommand::PushClip(_) => {
            depths.clips = depths
                .clips
                .checked_add(1)
                .ok_or_else(|| Error::compile("paint_ir", "clip stack overflow"))?
        }
        PaintCommand::PopClip => pop(&mut depths.clips, "clip")?,
        PaintCommand::PushTransform(_) => {
            depths.transforms = depths
                .transforms
                .checked_add(1)
                .ok_or_else(|| Error::compile("paint_ir", "transform stack overflow"))?
        }
        PaintCommand::PopTransform => pop(&mut depths.transforms, "transform")?,
        PaintCommand::BeginLayer(_) => {
            depths.layers = depths
                .layers
                .checked_add(1)
                .ok_or_else(|| Error::compile("paint_ir", "layer stack overflow"))?
        }
        PaintCommand::EndLayer => pop(&mut depths.layers, "layer")?,
        _ => {}
    }
    Ok(())
}

fn pop(value: &mut usize, stack: &'static str) -> Result<()> {
    if *value == 0 {
        return Err(Error::compile(
            "paint_ir",
            format!("{stack} stack underflow"),
        ));
    }
    *value -= 1;
    Ok(())
}

fn snapshot_commands(commands: &[PaintCommand]) -> PaintSnapshot {
    let mut text = String::new();
    let mut fingerprint = 0xcbf29ce484222325_u64;
    for (index, command) in commands.iter().enumerate() {
        let line = format!("{index}:{}\n", command.stable_debug());
        for byte in line.bytes() {
            fingerprint ^= u64::from(byte);
            fingerprint = fingerprint.wrapping_mul(0x100000001b3);
        }
        text.push_str(&line);
    }
    PaintSnapshot {
        text,
        command_count: commands.len(),
        fingerprint,
    }
}

fn validate_opacity(field: &'static str, opacity: f32) -> Result<()> {
    if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
        return Err(Error::invalid_input(
            Some(field.to_owned()),
            "must be finite and between zero and one",
        ));
    }
    Ok(())
}

fn valid_handle(field: &'static str, valid: bool) -> Result<()> {
    if !valid {
        return Err(Error::invalid_input(
            Some(field.to_owned()),
            "generation zero is invalid",
        ));
    }
    Ok(())
}

fn rect_text(rect: Rect) -> String {
    format!(
        "[{:.6},{:.6},{:.6},{:.6}]",
        rect.origin.x, rect.origin.y, rect.size.width, rect.size.height
    )
}
fn radii_text(r: CornerRadii) -> String {
    format!(
        "[{:.6},{:.6},{:.6},{:.6}]",
        r.top_left, r.top_right, r.bottom_right, r.bottom_left
    )
}
fn color_text(c: Color) -> String {
    format!("#{:02X}{:02X}{:02X}{:02X}", c.red, c.green, c.blue, c.alpha)
}
fn paint_text(p: &Paint) -> String {
    let brush = match &p.brush {
        Brush::Solid(c) => color_text(*c),
        Brush::LinearGradient(g) => format!("linear_gradient stops={}", g.stops.len()),
    };
    format!(
        "brush={brush} opacity={:.6} blend={:?} shadow={:?}",
        p.opacity, p.blend_mode, p.shadow
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canvas_is_atomic_and_stack_checked() {
        let mut canvas = Canvas::new();
        canvas
            .fill_rect(Rect::from_xywh(0.0, 0.0, 10.0, 10.0), Color::WHITE)
            .unwrap();
        let before = canvas.commands().to_vec();
        assert!(canvas.replay(&[PaintCommand::PopClip]).is_err());
        assert_eq!(canvas.commands(), before);
        canvas
            .begin_layer(LayerSpec::new(Rect::from_xywh(0.0, 0.0, 20.0, 20.0)))
            .unwrap();
        assert!(canvas.snapshot().is_err());
        canvas.end_layer().unwrap();
        assert_eq!(canvas.snapshot().unwrap().command_count(), 3);
    }
    #[test]
    fn path_is_aggregated_and_requires_move_to() {
        assert!(Path::new([PathSegment::LineTo(Point::ZERO)], FillRule::NonZero).is_err());
        let path = Path::new(
            [
                PathSegment::MoveTo(Point::ZERO),
                PathSegment::LineTo(Point::new(4.0, 4.0)),
            ],
            FillRule::NonZero,
        )
        .unwrap();
        assert_eq!(path.segments().len(), 2);
    }
}
