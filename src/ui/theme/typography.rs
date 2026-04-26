pub use crate::text::font::FontWeight;
use crate::ui::unit::{sp, Sp};

#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub font_family: Option<String>,
    pub size: Sp,
    pub line_height: Option<Sp>,
    pub weight: FontWeight,
    pub letter_spacing: Option<Sp>,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: None,
            size: sp(16.0),
            line_height: Some(sp(22.0)),
            weight: FontWeight::Regular,
            letter_spacing: Some(sp(0.0)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypeScale {
    pub display: TextStyle,
    pub headline: TextStyle,
    pub title: TextStyle,
    pub body: TextStyle,
    pub body_small: TextStyle,
    pub label: TextStyle,
    pub label_small: TextStyle,
    pub code: TextStyle,
}

impl Default for TypeScale {
    fn default() -> Self {
        Self {
            display: TextStyle {
                size: sp(40.0),
                line_height: Some(sp(48.0)),
                weight: FontWeight::Bold,
                ..TextStyle::default()
            },
            headline: TextStyle {
                size: sp(28.0),
                line_height: Some(sp(34.0)),
                weight: FontWeight::SemiBold,
                ..TextStyle::default()
            },
            title: TextStyle {
                size: sp(20.0),
                line_height: Some(sp(28.0)),
                weight: FontWeight::SemiBold,
                ..TextStyle::default()
            },
            body: TextStyle::default(),
            body_small: TextStyle {
                size: sp(14.0),
                line_height: Some(sp(20.0)),
                ..TextStyle::default()
            },
            label: TextStyle {
                size: sp(14.0),
                line_height: Some(sp(18.0)),
                weight: FontWeight::Medium,
                ..TextStyle::default()
            },
            label_small: TextStyle {
                size: sp(12.0),
                line_height: Some(sp(16.0)),
                weight: FontWeight::Medium,
                ..TextStyle::default()
            },
            code: TextStyle {
                font_family: Some("monospace".to_string()),
                size: sp(14.0),
                line_height: Some(sp(20.0)),
                ..TextStyle::default()
            },
        }
    }
}
