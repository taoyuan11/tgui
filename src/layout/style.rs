use crate::core::{Error, Result, Size};

/// A preferred, minimum, maximum, or flex-basis dimension in logical pixels.
/// Percentages use a `0.0..=1.0` fraction (`0.5` is fifty percent).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Dimension {
    #[default]
    Auto,
    Length(f32),
    Percent(f32),
}

impl Dimension {
    pub const fn length(value: f32) -> Self {
        Self::Length(value)
    }

    pub const fn percent(value: f32) -> Self {
        Self::Percent(value)
    }
}

/// A non-auto box-model length in logical pixels or as a fraction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LengthPercentage {
    Length(f32),
    Percent(f32),
}

impl LengthPercentage {
    pub const ZERO: Self = Self::Length(0.0);

    pub const fn length(value: f32) -> Self {
        Self::Length(value)
    }

    pub const fn percent(value: f32) -> Self {
        Self::Percent(value)
    }
}

impl Default for LengthPercentage {
    fn default() -> Self {
        Self::ZERO
    }
}

/// An inset or margin length that may be automatic.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum LengthPercentageAuto {
    #[default]
    Auto,
    Length(f32),
    Percent(f32),
}

impl LengthPercentageAuto {
    pub const fn length(value: f32) -> Self {
        Self::Length(value)
    }

    pub const fn percent(value: f32) -> Self {
        Self::Percent(value)
    }
}

/// Width/height pair used by layout styles without conflating it with a
/// resolved [`Size`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutSize<T> {
    pub width: T,
    pub height: T,
}

impl<T> LayoutSize<T> {
    pub const fn new(width: T, height: T) -> Self {
        Self { width, height }
    }
}

/// Logical left/right/top/bottom values.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sides<T> {
    pub left: T,
    pub right: T,
    pub top: T,
    pub bottom: T,
}

impl<T: Copy> Sides<T> {
    pub const fn all(value: T) -> Self {
        Self {
            left: value,
            right: value,
            top: value,
            bottom: value,
        }
    }

    pub const fn horizontal_vertical(horizontal: T, vertical: T) -> Self {
        Self {
            left: horizontal,
            right: horizontal,
            top: vertical,
            bottom: vertical,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Display {
    Block,
    #[default]
    Flex,
    Grid,
    None,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Position {
    #[default]
    Relative,
    Absolute,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overflow {
    #[default]
    Visible,
    Clip,
    Hidden,
    Scroll,
}

impl Overflow {
    pub const fn clips(self) -> bool {
        !matches!(self, Self::Visible)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverflowAxes {
    pub x: Overflow,
    pub y: Overflow,
}

impl OverflowAxes {
    pub const VISIBLE: Self = Self::new(Overflow::Visible, Overflow::Visible);
    pub const HIDDEN: Self = Self::new(Overflow::Hidden, Overflow::Hidden);
    pub const SCROLL: Self = Self::new(Overflow::Scroll, Overflow::Scroll);

    pub const fn new(x: Overflow, y: Overflow) -> Self {
        Self { x, y }
    }

    pub const fn clips(self) -> bool {
        self.x.clips() || self.y.clips()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignItems {
    Start,
    End,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
    Stretch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignContent {
    Start,
    End,
    FlexStart,
    FlexEnd,
    Center,
    Stretch,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// A minimal explicit grid-track definition.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum GridTrack {
    #[default]
    Auto,
    Length(f32),
    Percent(f32),
    Fraction(f32),
}

impl GridTrack {
    pub const fn length(value: f32) -> Self {
        Self::Length(value)
    }

    pub const fn percent(value: f32) -> Self {
        Self::Percent(value)
    }

    pub const fn fraction(value: f32) -> Self {
        Self::Fraction(value)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GridPlacement {
    #[default]
    Auto,
    Line(i16),
    Span(u16),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GridAxisPlacement {
    pub start: GridPlacement,
    pub end: GridPlacement,
}

impl GridAxisPlacement {
    pub const fn new(start: GridPlacement, end: GridPlacement) -> Self {
        Self { start, end }
    }
}

/// Boundaries cap propagation in the crate-private Dirty Tree. The root of a
/// window is always treated as all four boundary kinds even when these fields
/// are false.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayoutBoundaries {
    pub layout: bool,
    pub render: bool,
    pub hit_test: bool,
    pub semantics: bool,
}

impl LayoutBoundaries {
    pub const NONE: Self = Self {
        layout: false,
        render: false,
        hit_test: false,
        semantics: false,
    };
    pub const ALL: Self = Self {
        layout: true,
        render: true,
        hit_test: true,
        semantics: true,
    };
}

/// Public layout subset wrapped over Taffy.
///
/// All numeric values are logical pixels. DPI is deliberately absent from the
/// style: it participates in measurement cache identity but is applied to
/// physical rendering only by later stages.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutStyle {
    pub display: Display,
    pub position: Position,
    pub overflow: OverflowAxes,
    pub scrollbar_width: f32,
    pub inset: Sides<LengthPercentageAuto>,
    pub size: LayoutSize<Dimension>,
    pub min_size: LayoutSize<Dimension>,
    pub max_size: LayoutSize<Dimension>,
    pub aspect_ratio: Option<f32>,
    pub margin: Sides<LengthPercentageAuto>,
    pub padding: Sides<LengthPercentage>,
    pub border: Sides<LengthPercentage>,
    pub gap: LayoutSize<LengthPercentage>,
    pub align_items: Option<AlignItems>,
    pub align_self: Option<AlignItems>,
    pub align_content: Option<AlignContent>,
    pub justify_content: Option<AlignContent>,
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub flex_basis: Dimension,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub grid_template_rows: Vec<GridTrack>,
    pub grid_template_columns: Vec<GridTrack>,
    pub grid_row: GridAxisPlacement,
    pub grid_column: GridAxisPlacement,
}

impl LayoutStyle {
    pub fn flex() -> Self {
        Self::default()
    }

    pub fn grid() -> Self {
        Self {
            display: Display::Grid,
            ..Self::default()
        }
    }

    pub fn block() -> Self {
        Self {
            display: Display::Block,
            ..Self::default()
        }
    }

    pub fn with_size(mut self, width: Dimension, height: Dimension) -> Self {
        self.size = LayoutSize::new(width, height);
        self
    }

    pub fn with_flex_direction(mut self, direction: FlexDirection) -> Self {
        self.flex_direction = direction;
        self
    }

    pub fn with_overflow(mut self, overflow: OverflowAxes) -> Self {
        self.overflow = overflow;
        self
    }

    pub fn with_grid_columns(mut self, tracks: impl IntoIterator<Item = GridTrack>) -> Self {
        self.grid_template_columns = tracks.into_iter().collect();
        self
    }

    pub fn with_grid_rows(mut self, tracks: impl IntoIterator<Item = GridTrack>) -> Self {
        self.grid_template_rows = tracks.into_iter().collect();
        self
    }

    pub fn validate(&self) -> Result<()> {
        validate_dimension(self.size.width, "layout.size.width")?;
        validate_dimension(self.size.height, "layout.size.height")?;
        validate_dimension(self.min_size.width, "layout.min_size.width")?;
        validate_dimension(self.min_size.height, "layout.min_size.height")?;
        validate_dimension(self.max_size.width, "layout.max_size.width")?;
        validate_dimension(self.max_size.height, "layout.max_size.height")?;
        validate_dimension(self.flex_basis, "layout.flex_basis")?;
        validate_non_negative(self.scrollbar_width, "layout.scrollbar_width")?;
        validate_non_negative(self.flex_grow, "layout.flex_grow")?;
        validate_non_negative(self.flex_shrink, "layout.flex_shrink")?;
        if let Some(ratio) = self.aspect_ratio {
            if !ratio.is_finite() || ratio <= 0.0 {
                return Err(Error::invalid_input(
                    Some("layout.aspect_ratio".to_owned()),
                    "aspect ratio must be finite and greater than zero",
                ));
            }
        }
        for (value, field) in [
            (self.inset.left, "layout.inset.left"),
            (self.inset.right, "layout.inset.right"),
            (self.inset.top, "layout.inset.top"),
            (self.inset.bottom, "layout.inset.bottom"),
            (self.margin.left, "layout.margin.left"),
            (self.margin.right, "layout.margin.right"),
            (self.margin.top, "layout.margin.top"),
            (self.margin.bottom, "layout.margin.bottom"),
        ] {
            validate_auto_length(value, field)?;
        }
        for (value, field) in [
            (self.padding.left, "layout.padding.left"),
            (self.padding.right, "layout.padding.right"),
            (self.padding.top, "layout.padding.top"),
            (self.padding.bottom, "layout.padding.bottom"),
            (self.border.left, "layout.border.left"),
            (self.border.right, "layout.border.right"),
            (self.border.top, "layout.border.top"),
            (self.border.bottom, "layout.border.bottom"),
            (self.gap.width, "layout.gap.width"),
            (self.gap.height, "layout.gap.height"),
        ] {
            validate_non_negative_length(value, field)?;
        }
        for (index, track) in self
            .grid_template_rows
            .iter()
            .chain(&self.grid_template_columns)
            .copied()
            .enumerate()
        {
            validate_grid_track(track, index)?;
        }
        validate_grid_axis(self.grid_row, "layout.grid_row")?;
        validate_grid_axis(self.grid_column, "layout.grid_column")?;
        Ok(())
    }

    pub(crate) fn fingerprint(&self) -> u64 {
        // This fingerprint is an in-process cache component, not a wire format.
        // Rust's derived Debug is deterministic for this value-only structure.
        fnv1a(format!("{self:?}").as_bytes())
    }

    pub(crate) fn to_taffy(&self) -> Result<taffy::Style> {
        self.validate()?;
        Ok(taffy::Style {
            display: match self.display {
                Display::Block => taffy::Display::Block,
                Display::Flex => taffy::Display::Flex,
                Display::Grid => taffy::Display::Grid,
                Display::None => taffy::Display::None,
            },
            position: match self.position {
                Position::Relative => taffy::Position::Relative,
                Position::Absolute => taffy::Position::Absolute,
            },
            overflow: taffy::geometry::Point {
                x: to_taffy_overflow(self.overflow.x),
                y: to_taffy_overflow(self.overflow.y),
            },
            scrollbar_width: self.scrollbar_width,
            inset: map_sides(self.inset, to_taffy_auto_length),
            size: map_layout_size(self.size, to_taffy_dimension),
            min_size: map_layout_size(self.min_size, to_taffy_dimension),
            max_size: map_layout_size(self.max_size, to_taffy_dimension),
            aspect_ratio: self.aspect_ratio,
            margin: map_sides(self.margin, to_taffy_auto_length),
            padding: map_sides(self.padding, to_taffy_length),
            border: map_sides(self.border, to_taffy_length),
            gap: map_layout_size(self.gap, to_taffy_length),
            align_items: self.align_items.map(to_taffy_align_items),
            align_self: self.align_self.map(to_taffy_align_items),
            align_content: self.align_content.map(to_taffy_align_content),
            justify_content: self.justify_content.map(to_taffy_align_content),
            flex_direction: match self.flex_direction {
                FlexDirection::Row => taffy::FlexDirection::Row,
                FlexDirection::Column => taffy::FlexDirection::Column,
                FlexDirection::RowReverse => taffy::FlexDirection::RowReverse,
                FlexDirection::ColumnReverse => taffy::FlexDirection::ColumnReverse,
            },
            flex_wrap: match self.flex_wrap {
                FlexWrap::NoWrap => taffy::FlexWrap::NoWrap,
                FlexWrap::Wrap => taffy::FlexWrap::Wrap,
                FlexWrap::WrapReverse => taffy::FlexWrap::WrapReverse,
            },
            flex_basis: to_taffy_dimension(self.flex_basis),
            flex_grow: self.flex_grow,
            flex_shrink: self.flex_shrink,
            grid_template_rows: self
                .grid_template_rows
                .iter()
                .copied()
                .map(to_taffy_grid_track)
                .collect(),
            grid_template_columns: self
                .grid_template_columns
                .iter()
                .copied()
                .map(to_taffy_grid_track)
                .collect(),
            grid_row: to_taffy_grid_axis(self.grid_row),
            grid_column: to_taffy_grid_axis(self.grid_column),
            ..taffy::Style::default()
        })
    }
}

impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            display: Display::Flex,
            position: Position::Relative,
            overflow: OverflowAxes::VISIBLE,
            scrollbar_width: 0.0,
            inset: Sides::all(LengthPercentageAuto::Auto),
            size: LayoutSize::new(Dimension::Auto, Dimension::Auto),
            min_size: LayoutSize::new(Dimension::Auto, Dimension::Auto),
            max_size: LayoutSize::new(Dimension::Auto, Dimension::Auto),
            aspect_ratio: None,
            margin: Sides::all(LengthPercentageAuto::Length(0.0)),
            padding: Sides::all(LengthPercentage::ZERO),
            border: Sides::all(LengthPercentage::ZERO),
            gap: LayoutSize::new(LengthPercentage::ZERO, LengthPercentage::ZERO),
            align_items: None,
            align_self: None,
            align_content: None,
            justify_content: None,
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::NoWrap,
            flex_basis: Dimension::Auto,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            grid_template_rows: Vec::new(),
            grid_template_columns: Vec::new(),
            grid_row: GridAxisPlacement::default(),
            grid_column: GridAxisPlacement::default(),
        }
    }
}

fn validate_dimension(value: Dimension, field: &'static str) -> Result<()> {
    match value {
        Dimension::Auto => Ok(()),
        Dimension::Length(value) => validate_non_negative(value, field),
        Dimension::Percent(value) => validate_fraction(value, field),
    }
}

fn validate_auto_length(value: LengthPercentageAuto, field: &'static str) -> Result<()> {
    match value {
        LengthPercentageAuto::Auto => Ok(()),
        LengthPercentageAuto::Length(value) => validate_finite(value, field),
        LengthPercentageAuto::Percent(value) => validate_fraction(value, field),
    }
}

fn validate_non_negative_length(value: LengthPercentage, field: &'static str) -> Result<()> {
    match value {
        LengthPercentage::Length(value) => validate_non_negative(value, field),
        LengthPercentage::Percent(value) => validate_fraction(value, field),
    }
}

fn validate_grid_track(track: GridTrack, index: usize) -> Result<()> {
    match track {
        GridTrack::Auto => Ok(()),
        GridTrack::Length(value) | GridTrack::Fraction(value) => {
            validate_non_negative(value, &format!("layout.grid_track[{index}]"))
        }
        GridTrack::Percent(value) => {
            validate_fraction(value, &format!("layout.grid_track[{index}]"))
        }
    }
}

fn validate_grid_axis(axis: GridAxisPlacement, field: &'static str) -> Result<()> {
    for placement in [axis.start, axis.end] {
        match placement {
            GridPlacement::Line(0) => {
                return Err(Error::invalid_input(
                    Some(field.to_owned()),
                    "grid line zero is invalid",
                ));
            }
            GridPlacement::Span(0) => {
                return Err(Error::invalid_input(
                    Some(field.to_owned()),
                    "grid span must be greater than zero",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_finite(value: f32, field: &str) -> Result<()> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(Error::invalid_input(
            Some(field.to_owned()),
            "layout value must be finite",
        ))
    }
}

fn validate_non_negative(value: f32, field: &str) -> Result<()> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(Error::invalid_input(
            Some(field.to_owned()),
            "layout value must be finite and non-negative",
        ))
    }
}

fn validate_fraction(value: f32, field: &str) -> Result<()> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(Error::invalid_input(
            Some(field.to_owned()),
            "percentage must be a finite fraction in the range 0..=1",
        ))
    }
}

fn map_sides<T, U>(value: Sides<T>, map: impl Fn(T) -> U) -> taffy::geometry::Rect<U> {
    taffy::geometry::Rect {
        left: map(value.left),
        right: map(value.right),
        top: map(value.top),
        bottom: map(value.bottom),
    }
}

fn map_layout_size<T, U>(value: LayoutSize<T>, map: impl Fn(T) -> U) -> taffy::geometry::Size<U> {
    taffy::geometry::Size {
        width: map(value.width),
        height: map(value.height),
    }
}

fn to_taffy_dimension(value: Dimension) -> taffy::Dimension {
    match value {
        Dimension::Auto => taffy::style_helpers::auto(),
        Dimension::Length(value) => taffy::style_helpers::length(value),
        Dimension::Percent(value) => taffy::style_helpers::percent(value),
    }
}

fn to_taffy_length(value: LengthPercentage) -> taffy::LengthPercentage {
    match value {
        LengthPercentage::Length(value) => taffy::style_helpers::length(value),
        LengthPercentage::Percent(value) => taffy::style_helpers::percent(value),
    }
}

fn to_taffy_auto_length(value: LengthPercentageAuto) -> taffy::LengthPercentageAuto {
    match value {
        LengthPercentageAuto::Auto => taffy::style_helpers::auto(),
        LengthPercentageAuto::Length(value) => taffy::style_helpers::length(value),
        LengthPercentageAuto::Percent(value) => taffy::style_helpers::percent(value),
    }
}

fn to_taffy_overflow(value: Overflow) -> taffy::Overflow {
    match value {
        Overflow::Visible => taffy::Overflow::Visible,
        Overflow::Clip => taffy::Overflow::Clip,
        Overflow::Hidden => taffy::Overflow::Hidden,
        Overflow::Scroll => taffy::Overflow::Scroll,
    }
}

fn to_taffy_align_items(value: AlignItems) -> taffy::AlignItems {
    match value {
        AlignItems::Start => taffy::AlignItems::START,
        AlignItems::End => taffy::AlignItems::END,
        AlignItems::FlexStart => taffy::AlignItems::FLEX_START,
        AlignItems::FlexEnd => taffy::AlignItems::FLEX_END,
        AlignItems::Center => taffy::AlignItems::CENTER,
        AlignItems::Baseline => taffy::AlignItems::BASELINE,
        AlignItems::Stretch => taffy::AlignItems::STRETCH,
    }
}

fn to_taffy_align_content(value: AlignContent) -> taffy::AlignContent {
    match value {
        AlignContent::Start => taffy::AlignContent::START,
        AlignContent::End => taffy::AlignContent::END,
        AlignContent::FlexStart => taffy::AlignContent::FLEX_START,
        AlignContent::FlexEnd => taffy::AlignContent::FLEX_END,
        AlignContent::Center => taffy::AlignContent::CENTER,
        AlignContent::Stretch => taffy::AlignContent::STRETCH,
        AlignContent::SpaceBetween => taffy::AlignContent::SPACE_BETWEEN,
        AlignContent::SpaceAround => taffy::AlignContent::SPACE_AROUND,
        AlignContent::SpaceEvenly => taffy::AlignContent::SPACE_EVENLY,
    }
}

fn to_taffy_grid_track(track: GridTrack) -> taffy::GridTemplateComponent<String> {
    match track {
        GridTrack::Auto => taffy::style_helpers::auto(),
        GridTrack::Length(value) => taffy::style_helpers::length(value),
        GridTrack::Percent(value) => taffy::style_helpers::percent(value),
        GridTrack::Fraction(value) => taffy::style_helpers::fr(value),
    }
}

fn to_taffy_grid_placement(placement: GridPlacement) -> taffy::GridPlacement<String> {
    match placement {
        GridPlacement::Auto => taffy::style_helpers::auto(),
        GridPlacement::Line(value) => taffy::style_helpers::line(value),
        GridPlacement::Span(value) => taffy::style_helpers::span(value),
    }
}

fn to_taffy_grid_axis(
    axis: GridAxisPlacement,
) -> taffy::geometry::Line<taffy::GridPlacement<String>> {
    taffy::geometry::Line {
        start: to_taffy_grid_placement(axis.start),
        end: to_taffy_grid_placement(axis.end),
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub(crate) fn validate_viewport(viewport: Size) -> Result<()> {
    viewport.validate().map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_numeric_styles_are_rejected_before_taffy() {
        let style = LayoutStyle {
            flex_grow: f32::NAN,
            ..LayoutStyle::default()
        };
        assert!(style.to_taffy().is_err());

        let mut style = LayoutStyle::grid();
        style.grid_template_columns = vec![GridTrack::Fraction(-1.0)];
        assert!(style.validate().is_err());
    }

    #[test]
    fn percentages_are_fractional_and_bounded_in_every_box_model_position() {
        let mut style = LayoutStyle::default();
        style.size.width = Dimension::Percent(1.0);
        style.min_size.height = Dimension::Percent(0.0);
        style.inset.left = LengthPercentageAuto::Percent(0.5);
        style.margin.right = LengthPercentageAuto::Percent(1.0);
        style.padding.top = LengthPercentage::Percent(0.25);
        style.border.bottom = LengthPercentage::Percent(0.0);
        style.gap.width = LengthPercentage::Percent(1.0);
        style.grid_template_columns = vec![GridTrack::Percent(0.0), GridTrack::Percent(1.0)];
        assert!(style.validate().is_ok());

        let mut invalid = style.clone();
        invalid.size.width = Dimension::Percent(1.000_001);
        assert!(invalid.validate().is_err());
        invalid.size.width = Dimension::Percent(-0.000_001);
        assert!(invalid.validate().is_err());
        invalid.size.width = Dimension::Percent(f32::NAN);
        assert!(invalid.validate().is_err());

        invalid = style.clone();
        invalid.inset.left = LengthPercentageAuto::Percent(2.0);
        assert!(invalid.validate().is_err());
        invalid = style.clone();
        invalid.margin.left = LengthPercentageAuto::Percent(f32::INFINITY);
        assert!(invalid.validate().is_err());
        invalid = style.clone();
        invalid.padding.left = LengthPercentage::Percent(2.0);
        assert!(invalid.validate().is_err());
        invalid = style.clone();
        invalid.border.left = LengthPercentage::Percent(-1.0);
        assert!(invalid.validate().is_err());
        invalid = style.clone();
        invalid.gap.width = LengthPercentage::Percent(2.0);
        assert!(invalid.validate().is_err());
        invalid = style;
        invalid.grid_template_columns = vec![GridTrack::Percent(2.0)];
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn grid_fractions_are_not_percentages() {
        let mut style = LayoutStyle::grid();
        style.grid_template_columns = vec![GridTrack::Fraction(2.0)];
        assert!(style.validate().is_ok());
        style.grid_template_columns = vec![GridTrack::Fraction(f32::INFINITY)];
        assert!(style.validate().is_err());
    }
}
