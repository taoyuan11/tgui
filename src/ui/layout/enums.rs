/// 交叉轴对齐方式。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

/// 主轴分布方式。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// 布局定位模式。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PositionType {
    #[default]
    Relative,
    Absolute,
}

/// 线性布局的主轴方向。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// 弹性布局的换行策略。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Wrap {
    #[default]
    NoWrap,
    Wrap,
}

/// 内容溢出时的可视化策略。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Overflow {
    Visible,
    #[default]
    Hidden,
    Scroll,
}
