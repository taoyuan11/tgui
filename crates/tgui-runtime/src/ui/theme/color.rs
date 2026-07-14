use crate::foundation::color::Color;

#[derive(Clone, Debug, PartialEq)]
pub struct ColorScheme {
    /// 应用或窗口的根背景色。
    /// 用于最外层画布、页面背景，以及所有 surface 背后的底色。
    pub background: Color,
    /// 直接绘制在 `background` 上的前景色。
    /// 用于放在页面背景上的文字、图标，而不是放在某个面板或控件表面上的内容。
    pub on_background: Color,
    /// 默认容器表面色。
    /// 用于卡片、面板、普通容器，以及大多数组件主体的背景。
    pub surface: Color,
    /// 更弱一级的容器表面色。
    /// 用于凹陷区域、静止状态的输入框背景，或需要比 `surface` 更低存在感的表面。
    pub surface_low: Color,
    /// 更强一级的容器表面色。
    /// 用于次级按钮填充、强调型面板，或需要比 `surface` 更突出一点的表面。
    pub surface_high: Color,
    /// 浮层表面色，通常带一点透明度。
    /// 用于对话框、弹出层、提示框等悬浮在普通内容之上的界面层。
    pub surface_overlay: Color,
    /// 放在 `surface` 系列表面上的主前景色。
    /// 用于正文、图标、输入框文本等主要内容。
    pub on_surface: Color,
    /// 放在 `surface` 系列表面上的弱化前景色。
    /// 用于占位符、辅助说明、副标题、弱化图标等次要内容。
    pub on_surface_muted: Color,
    /// 品牌色或主强调色。
    /// 用于主按钮、选中态/勾选态控件、链接、关键高亮等最重要的交互强调。
    pub primary: Color,
    /// 放在 `primary` 上的前景色。
    /// 用于主按钮、强调徽标、选中态高亮块上的文字和图标。
    pub on_primary: Color,
    /// 柔和版强调容器色。
    /// 用于主强调元素的悬浮背景、强调容器背景，或不适合直接使用 `primary` 的弱化强调面。
    pub primary_container: Color,
    /// 放在 `primary_container` 上的前景色。
    /// 用于柔和强调背景上的文字和图标。
    pub on_primary_container: Color,
    /// 成功语义色。
    /// 用于完成、通过、健康状态、确认成功等正向反馈。
    pub success: Color,
    /// 放在 `success` 上的前景色。
    /// 用于成功标签、成功提示条、成功状态块上的文字和图标。
    pub on_success: Color,
    /// 警告语义色。
    /// 用于提醒、待处理、需要注意但不是致命错误的状态。
    pub warning: Color,
    /// 放在 `warning` 上的前景色。
    /// 用于警告标签、警告提示条、警告状态块上的文字和图标。
    pub on_warning: Color,
    /// 错误或危险语义色。
    /// 用于危险按钮、校验错误、失败状态、破坏性操作等场景。
    pub error: Color,
    /// 放在 `error` 上的前景色。
    /// 用于错误提示、危险标签、错误状态块上的文字和图标。
    pub on_error: Color,
    /// 标准描边或分隔线颜色。
    /// 用于默认边框、分割线、控件轮廓线。
    pub outline: Color,
    /// 更弱的描边或分隔线颜色。
    /// 用于更轻的边框、面板描边、弱化分隔线等低存在感轮廓。
    pub outline_muted: Color,
    /// 高可见性的聚焦指示色。
    /// 用于聚焦边框、focus ring，以及键盘导航时的焦点提示。
    pub focus_ring: Color,
    /// 文本选择高亮色。
    /// 用于输入框文本选中背景，或类似的临时选择高亮。
    pub selection: Color,
    /// 禁用态表面或填充色。
    /// 用于禁用组件的背景、边框、轨道等需要表现“不可用”的区域。
    pub disabled: Color,
    /// 放在 `disabled` 上的前景色。
    /// 用于禁用态文字、图标、光标，或任何显示在禁用背景上的内容。
    pub on_disabled: Color,
    /// 背景遮罩色。
    /// 用于模态框 scrim，或任何需要压暗后方内容的全屏遮罩层。
    pub scrim: Color,
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self::dark()
    }
}

impl ColorScheme {
    pub fn light() -> Self {
        let primary = Color::hexa(0x2563EBFF);
        Self {
            background: Color::hexa(0xFAFAFAFF),
            on_background: Color::hexa(0x09090BFF),
            surface: Color::hexa(0xFFFFFFFF),
            surface_low: Color::hexa(0xF4F4F5FF),
            surface_high: Color::hexa(0xE4E4E7FF),
            surface_overlay: Color::hexa(0xFFFFFFFA),
            on_surface: Color::hexa(0x18181BFF),
            on_surface_muted: Color::hexa(0x71717AFF),
            primary,
            on_primary: Color::hexa(0xFFFFFFFF),
            primary_container: Color::hexa(0xDBEAFEFF),
            on_primary_container: Color::hexa(0x1E3A8AFF),
            success: Color::hexa(0x10B981FF),
            on_success: Color::hexa(0x09090BFF),
            warning: Color::hexa(0xF59E0BFF),
            on_warning: Color::hexa(0x09090BFF),
            error: Color::hexa(0xEF4444FF),
            on_error: Color::hexa(0xFAFAFAFF),
            outline: Color::hexa(0xD4D4D8FF),
            outline_muted: Color::hexa(0xE4E4E780),
            focus_ring: primary.with_alpha_factor(0.40),
            selection: primary.with_alpha_factor(0.18),
            disabled: Color::hexa(0xF4F4F5FF),
            on_disabled: Color::hexa(0xA1A1AAFF),
            scrim: Color::hexa(0x09090B66),
        }
    }

    pub fn dark() -> Self {
        let primary = Color::hexa(0x60A5FAFF);
        Self {
            background: Color::hexa(0x09090BFF),
            on_background: Color::hexa(0xFAFAFAFF),
            surface: Color::hexa(0x18181BFF),
            surface_low: Color::hexa(0x27272AFF),
            surface_high: Color::hexa(0x3F3F46FF),
            surface_overlay: Color::hexa(0x18181BFA),
            on_surface: Color::hexa(0xF4F4F5FF),
            on_surface_muted: Color::hexa(0xA1A1AAFF),
            primary,
            on_primary: Color::hexa(0x08111FFF),
            primary_container: Color::hexa(0x172554FF),
            on_primary_container: Color::hexa(0xDBEAFEFF),
            success: Color::hexa(0x34D399FF),
            on_success: Color::hexa(0x09090BFF),
            warning: Color::hexa(0xFBBF24FF),
            on_warning: Color::hexa(0x09090BFF),
            error: Color::hexa(0xF87171FF),
            on_error: Color::hexa(0x09090BFF),
            outline: Color::hexa(0x52525BFF),
            outline_muted: Color::hexa(0x3F3F4680),
            focus_ring: primary.with_alpha_factor(0.50),
            selection: primary.with_alpha_factor(0.24),
            disabled: Color::hexa(0x27272AFF),
            on_disabled: Color::hexa(0x71717AFF),
            scrim: Color::hexa(0x000000CC),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_schemes_match_neutral_token_table() {
        let light = ColorScheme::light();
        assert_eq!(light.background, Color::hexa(0xFAFAFAFF));
        assert_eq!(light.on_background, Color::hexa(0x09090BFF));
        assert_eq!(light.surface, Color::hexa(0xFFFFFFFF));
        assert_eq!(light.surface_low, Color::hexa(0xF4F4F5FF));
        assert_eq!(light.surface_high, Color::hexa(0xE4E4E7FF));
        assert_eq!(light.surface_overlay, Color::hexa(0xFFFFFFFA));
        assert_eq!(light.primary, Color::hexa(0x2563EBFF));
        assert_eq!(light.on_primary, Color::WHITE);
        assert_eq!(light.primary_container, Color::hexa(0xDBEAFEFF));
        assert_eq!(light.on_primary_container, Color::hexa(0x1E3A8AFF));
        assert_eq!(light.focus_ring, Color::hexa(0x2563EB66));
        assert_eq!(light.selection, Color::hexa(0x2563EB2E));

        let dark = ColorScheme::dark();
        assert_eq!(dark.background, Color::hexa(0x09090BFF));
        assert_eq!(dark.on_background, Color::hexa(0xFAFAFAFF));
        assert_eq!(dark.surface, Color::hexa(0x18181BFF));
        assert_eq!(dark.surface_low, Color::hexa(0x27272AFF));
        assert_eq!(dark.surface_high, Color::hexa(0x3F3F46FF));
        assert_eq!(dark.surface_overlay, Color::hexa(0x18181BFA));
        assert_eq!(dark.primary, Color::hexa(0x60A5FAFF));
        assert_eq!(dark.on_primary, Color::hexa(0x08111FFF));
        assert_eq!(dark.primary_container, Color::hexa(0x172554FF));
        assert_eq!(dark.on_primary_container, Color::hexa(0xDBEAFEFF));
        assert_eq!(dark.focus_ring, Color::hexa(0x60A5FA80));
        assert_eq!(dark.selection, Color::hexa(0x60A5FA3D));
    }
}
