use crate::foundation::color::Color;

#[derive(Clone, Debug, PartialEq)]
pub struct ColorScheme {
    pub background: Color,
    pub on_background: Color,
    pub surface: Color,
    pub surface_low: Color,
    pub surface_high: Color,
    pub surface_overlay: Color,
    pub on_surface: Color,
    pub on_surface_muted: Color,
    pub primary: Color,
    pub on_primary: Color,
    pub primary_container: Color,
    pub on_primary_container: Color,
    pub success: Color,
    pub on_success: Color,
    pub warning: Color,
    pub on_warning: Color,
    pub error: Color,
    pub on_error: Color,
    pub outline: Color,
    pub outline_muted: Color,
    pub focus_ring: Color,
    pub selection: Color,
    pub disabled: Color,
    pub on_disabled: Color,
    pub scrim: Color,
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self::dark()
    }
}

impl ColorScheme {
    pub fn light() -> Self {
        Self {
            background: Color::hexa(0xF6F8FCFF),
            on_background: Color::hexa(0x171B22FF),
            surface: Color::hexa(0xFFFFFFFF),
            surface_low: Color::hexa(0xEEF2F8FF),
            surface_high: Color::hexa(0xE2E8F2FF),
            surface_overlay: Color::hexa(0xFFFFFFF2),
            on_surface: Color::hexa(0x18202AFF),
            on_surface_muted: Color::hexa(0x5C6773E0),
            primary: Color::hexa(0x2F6FEBFF),
            on_primary: Color::hexa(0xFFFFFFFF),
            primary_container: Color::hexa(0xDCE8FFFF),
            on_primary_container: Color::hexa(0x0E2A5FFF),
            success: Color::hexa(0x1F9D61FF),
            on_success: Color::hexa(0xFFFFFFFF),
            warning: Color::hexa(0xC77B12FF),
            on_warning: Color::hexa(0xFFFFFFFF),
            error: Color::hexa(0xFF4D4FFF),
            on_error: Color::hexa(0xFFFFFFFF),
            outline: Color::hexa(0xC1C9D6FF),
            outline_muted: Color::hexa(0xD6DDE7CC),
            focus_ring: Color::hexa(0x5B8CFFFF),
            selection: Color::hexa(0x2F6FEB59),
            disabled: Color::hexa(0xD8DEE8FF),
            on_disabled: Color::hexa(0x8792A2FF),
            scrim: Color::hexa(0x11182766),
        }
    }

    pub fn dark() -> Self {
        Self {
            background: Color::hexa(0x181A20FF),
            on_background: Color::hexa(0xEFF2F8FF),
            surface: Color::hexa(0x20242CFF),
            surface_low: Color::hexa(0x272C35FF),
            surface_high: Color::hexa(0x313743FF),
            surface_overlay: Color::hexa(0x2A2F39F2),
            on_surface: Color::hexa(0xF0F2F7FF),
            on_surface_muted: Color::hexa(0xBAC2CFD9),
            primary: Color::hexa(0x2F6FEBFF),
            on_primary: Color::hexa(0xFFFFFFFF),
            primary_container: Color::hexa(0x1F3D73FF),
            on_primary_container: Color::hexa(0xDCE8FFFF),
            success: Color::hexa(0x30C27CFF),
            on_success: Color::hexa(0x072111FF),
            warning: Color::hexa(0xF3B248FF),
            on_warning: Color::hexa(0x2A1700FF),
            error: Color::hexa(0xFF4D4FFF),
            on_error: Color::hexa(0xFFFFFFFF),
            outline: Color::hexa(0x4A5261FF),
            outline_muted: Color::hexa(0x6C7687A6),
            focus_ring: Color::hexa(0x73A4FFFF),
            selection: Color::hexa(0x73A4FF66),
            disabled: Color::hexa(0x3A414FFF),
            on_disabled: Color::hexa(0x95A0B2FF),
            scrim: Color::hexa(0x05070BCC),
        }
    }
}
