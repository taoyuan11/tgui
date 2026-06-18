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
        Self {
            background: Color::hexa(0xF7F9FCFF),
            on_background: Color::hexa(0x1B1F24FF),
            surface: Color::hexa(0xFFFFFFFF),
            surface_low: Color::hexa(0xF3F6FAFF),
            surface_high: Color::hexa(0xEEF3F8FF),
            surface_overlay: Color::hexa(0xFFFFFFF5),
            on_surface: Color::hexa(0x202427FF),
            on_surface_muted: Color::hexa(0x5F6B7AFF),
            primary: Color::hexa(0x0F6CBDFF),
            on_primary: Color::hexa(0xFFFFFFFF),
            primary_container: Color::hexa(0xE4F2FFFF),
            on_primary_container: Color::hexa(0x043862FF),
            success: Color::hexa(0x0F9F6EFF),
            on_success: Color::hexa(0xFFFFFFFF),
            warning: Color::hexa(0xD98600FF),
            on_warning: Color::hexa(0x2F1D00FF),
            error: Color::hexa(0xD92D20FF),
            on_error: Color::hexa(0xFFFFFFFF),
            outline: Color::hexa(0xD2DAE3FF),
            outline_muted: Color::hexa(0xE2E8F0B8),
            focus_ring: Color::hexa(0x3A96DDFF),
            selection: Color::hexa(0x0F6CBD4D),
            disabled: Color::hexa(0xEDF1F5FF),
            on_disabled: Color::hexa(0x9AA6B2FF),
            scrim: Color::hexa(0x10182066),
        }
    }

    pub fn dark() -> Self {
        Self {
            background: Color::hexa(0x101418FF),
            on_background: Color::hexa(0xF4F7FBFF),
            surface: Color::hexa(0x171C22FF),
            surface_low: Color::hexa(0x12171DFF),
            surface_high: Color::hexa(0x202832FF),
            surface_overlay: Color::hexa(0x202832F5),
            on_surface: Color::hexa(0xF3F6FAFF),
            on_surface_muted: Color::hexa(0xA8B4C2E6),
            primary: Color::hexa(0x4EA5F6FF),
            on_primary: Color::hexa(0xFFFFFFFF),
            primary_container: Color::hexa(0x123B5EFF),
            on_primary_container: Color::hexa(0xD8ECFFFF),
            success: Color::hexa(0x4CC38AFF),
            on_success: Color::hexa(0x061E14FF),
            warning: Color::hexa(0xE9A441FF),
            on_warning: Color::hexa(0x2B1B00FF),
            error: Color::hexa(0xFF6B66FF),
            on_error: Color::hexa(0xFFFFFFFF),
            outline: Color::hexa(0x3A4654FF),
            outline_muted: Color::hexa(0x2D3744B8),
            focus_ring: Color::hexa(0x78BDF8FF),
            selection: Color::hexa(0x4EA5F666),
            disabled: Color::hexa(0x252D36FF),
            on_disabled: Color::hexa(0x758292FF),
            scrim: Color::hexa(0x03070BCC),
        }
    }
}
