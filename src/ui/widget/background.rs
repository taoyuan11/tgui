use crate::foundation::binding::Binding;
use crate::foundation::color::Color;
use crate::media::{ContentFit, MediaBytes, MediaSource};
use crate::ui::layout::Value;
use crate::ui::unit::Dp;

use super::common::Point;

const MAX_BACKGROUND_GRADIENT_STOPS: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BackgroundGradientStop {
    pub offset: f32,
    pub color: Color,
}

impl BackgroundGradientStop {
    pub fn new(offset: f32, color: Color) -> Self {
        Self {
            offset: offset.clamp(0.0, 1.0),
            color,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundLinearGradient {
    pub start: Point,
    pub end: Point,
    pub stops: Vec<BackgroundGradientStop>,
}

impl BackgroundLinearGradient {
    pub fn new(
        start: impl Into<Point>,
        end: impl Into<Point>,
        stops: impl Into<Vec<BackgroundGradientStop>>,
    ) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
            stops: clamp_background_stops(stops.into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundRadialGradient {
    pub center: Point,
    pub radius: Dp,
    pub stops: Vec<BackgroundGradientStop>,
}

impl BackgroundRadialGradient {
    pub fn new(
        center: impl Into<Point>,
        radius: impl Into<Dp>,
        stops: impl Into<Vec<BackgroundGradientStop>>,
    ) -> Self {
        Self {
            center: center.into(),
            radius: radius.into(),
            stops: clamp_background_stops(stops.into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum BackgroundBrush {
    Solid(Color),
    LinearGradient(BackgroundLinearGradient),
    RadialGradient(BackgroundRadialGradient),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BackgroundImage {
    pub source: MediaSource,
    pub fit: ContentFit,
}

impl BackgroundImage {
    pub fn new(source: impl Into<MediaSource>) -> Self {
        Self {
            source: source.into(),
            fit: ContentFit::Cover,
        }
    }

    pub fn from_path(path: impl Into<std::path::PathBuf>) -> Self {
        Self::new(MediaSource::Path(path.into()))
    }

    pub fn from_url(url: impl Into<String>) -> Self {
        Self::new(MediaSource::Url(url.into()))
    }

    pub fn from_bytes(bytes: impl Into<MediaBytes>) -> Self {
        Self::new(MediaSource::Bytes(bytes.into()))
    }

    pub fn fit(mut self, fit: ContentFit) -> Self {
        self.fit = fit;
        self
    }
}

impl From<Color> for BackgroundBrush {
    fn from(value: Color) -> Self {
        Self::Solid(value)
    }
}

impl From<BackgroundLinearGradient> for BackgroundBrush {
    fn from(value: BackgroundLinearGradient) -> Self {
        Self::LinearGradient(value)
    }
}

impl From<BackgroundRadialGradient> for BackgroundBrush {
    fn from(value: BackgroundRadialGradient) -> Self {
        Self::RadialGradient(value)
    }
}

impl From<Color> for Value<BackgroundBrush> {
    fn from(value: Color) -> Self {
        Value::Static(BackgroundBrush::Solid(value))
    }
}

impl From<BackgroundLinearGradient> for Value<BackgroundBrush> {
    fn from(value: BackgroundLinearGradient) -> Self {
        Value::Static(BackgroundBrush::LinearGradient(value))
    }
}

impl From<BackgroundRadialGradient> for Value<BackgroundBrush> {
    fn from(value: BackgroundRadialGradient) -> Self {
        Value::Static(BackgroundBrush::RadialGradient(value))
    }
}

impl From<Binding<Color>> for Value<BackgroundBrush> {
    fn from(value: Binding<Color>) -> Self {
        Value::Bound(value.map(BackgroundBrush::Solid))
    }
}

impl From<Value<Color>> for Value<BackgroundBrush> {
    fn from(value: Value<Color>) -> Self {
        match value {
            Value::Static(color) => Value::Static(BackgroundBrush::Solid(color)),
            Value::Bound(binding) => Value::Bound(binding.map(BackgroundBrush::Solid)),
        }
    }
}

fn clamp_background_stops(mut stops: Vec<BackgroundGradientStop>) -> Vec<BackgroundGradientStop> {
    if stops.is_empty() {
        return vec![
            BackgroundGradientStop::new(0.0, Color::TRANSPARENT),
            BackgroundGradientStop::new(1.0, Color::TRANSPARENT),
        ];
    }

    stops.sort_by(|left, right| left.offset.total_cmp(&right.offset));
    if stops.len() > MAX_BACKGROUND_GRADIENT_STOPS {
        stops.truncate(MAX_BACKGROUND_GRADIENT_STOPS);
    }
    stops
}

#[cfg(test)]
mod tests {
    use crate::media::{ContentFit, MediaSource};

    use super::BackgroundImage;

    #[test]
    fn background_image_defaults_to_cover() {
        let image = BackgroundImage::from_path("assets/bg.jpg");

        assert_eq!(image.fit, ContentFit::Cover);
        assert_eq!(image.source, MediaSource::path("assets/bg.jpg"));
    }

    #[test]
    fn background_image_fit_is_configurable() {
        let image = BackgroundImage::from_url("https://example.com/bg.jpg").fit(ContentFit::Contain);

        assert_eq!(image.fit, ContentFit::Contain);
    }
}
