use crate::ui::unit::{dp, Dp};
use crate::ui::widget::common::Rect;

/// 浮层相对锚点的主轴方向。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

impl Side {
    /// 取相反方向（用于翻转策略）。
    pub fn opposite(self) -> Self {
        match self {
            Side::Top => Side::Bottom,
            Side::Bottom => Side::Top,
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }

    /// 是否为垂直方向（Top / Bottom）。垂直时主轴为 y，交叉轴为 x。
    pub fn is_vertical(self) -> bool {
        matches!(self, Side::Top | Side::Bottom)
    }
}

/// 浮层在交叉轴上的对齐方式。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    Start,
    Center,
    End,
}

/// 浮层位置 = 主轴方向 + 交叉轴对齐。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub side: Side,
    pub alignment: Alignment,
}

impl Placement {
    pub const fn new(side: Side, alignment: Alignment) -> Self {
        Self { side, alignment }
    }

    pub const fn top() -> Self {
        Self::new(Side::Top, Alignment::Center)
    }

    pub const fn bottom() -> Self {
        Self::new(Side::Bottom, Alignment::Center)
    }

    pub const fn left() -> Self {
        Self::new(Side::Left, Alignment::Center)
    }

    pub const fn right() -> Self {
        Self::new(Side::Right, Alignment::Center)
    }

    pub const fn align(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }
}

impl Default for Placement {
    fn default() -> Self {
        Self::bottom()
    }
}

/// 当主轴方向放不下浮层时的处理策略。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FlipPolicy {
    /// 不调整方向；若 `PlacementOptions::clamp_to_viewport` 为 true 则裁切到视口内。
    None,
    /// 仅在交叉轴上平移以贴住视口。
    ShiftOnly,
    /// 翻到反向，选择拟合度更高的一侧。
    #[default]
    FlipSide,
    /// 先翻方向，再在交叉轴上平移修正。
    FlipAndShift,
    /// 完全放不下时标记隐藏。
    Hide,
}

/// 浮层定位输入。所有像素值均为 Dp（逻辑像素）。
#[derive(Clone, Debug, PartialEq)]
pub struct PlacementOptions {
    pub placement: Placement,
    /// 主轴上锚点与浮层之间的间隙（Dp）。
    pub offset: Dp,
    /// 交叉轴上的额外偏移（Dp，允许负值）。
    pub cross_offset: Dp,
    pub flip: FlipPolicy,
    /// 浮层距视口边的最小留白（Dp）。
    pub viewport_padding: Dp,
    /// 在 `FlipPolicy::None` 下是否将溢出的浮层裁切回视口内。
    pub clamp_to_viewport: bool,
    /// 对 Top/Bottom 方向：用锚点宽度覆盖内容宽度（用于下拉框等场景）。
    pub match_anchor_width: bool,
}

impl Default for PlacementOptions {
    fn default() -> Self {
        Self {
            placement: Placement::bottom(),
            offset: dp(8.0),
            cross_offset: Dp::ZERO,
            flip: FlipPolicy::FlipSide,
            viewport_padding: dp(8.0),
            clamp_to_viewport: true,
            match_anchor_width: false,
        }
    }
}

/// 浮层求解结果。所有矩形均为窗口坐标系下的 Dp 值。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolvedPlacement {
    /// 浮层最终矩形。
    pub rect: Rect,
    /// 经翻转/对齐解析后的最终 placement。
    pub resolved_placement: Placement,
    /// 浮层有效裁剪矩形（一般为 rect 与视口的交集）。
    pub clip_rect: Rect,
    /// 是否发生了方向翻转。
    pub did_flip: bool,
    /// 是否因 `FlipPolicy::Hide` 而被判定隐藏。
    pub was_hidden: bool,
    /// 求解时使用的锚点矩形（Anchor::Point 时为零尺寸）。
    pub anchor_rect: Rect,
}

/// 浮层在 z-order 上的归属层级。低 -> 高：Tooltip / Popover / Menu / Modal。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum OverlayLayer {
    #[default]
    Tooltip,
    Popover,
    Menu,
    Modal,
}

impl OverlayLayer {
    /// 所有层级，从最底到最顶。`finalize_overlay_layers` 据此顺序合并 bucket。
    pub const ALL: [OverlayLayer; 4] = [
        OverlayLayer::Tooltip,
        OverlayLayer::Popover,
        OverlayLayer::Menu,
        OverlayLayer::Modal,
    ];

    /// 转换为 0..ALL.len() 的下标。
    pub(crate) const fn index(self) -> usize {
        match self {
            OverlayLayer::Tooltip => 0,
            OverlayLayer::Popover => 1,
            OverlayLayer::Menu => 2,
            OverlayLayer::Modal => 3,
        }
    }
}

/// 浮层稳定标识。由消费者（trigger widget）保证：同一逻辑浮层每帧 id 不变。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OverlayId(pub u64);

impl OverlayId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }
}
