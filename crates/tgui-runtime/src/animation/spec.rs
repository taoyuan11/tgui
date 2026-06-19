use std::time::Duration;

const DEFAULT_DURATION_MS: u64 = 180;

/// 动画缓动曲线。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationCurve {
    Linear,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
}

impl AnimationCurve {
    /// 根据 0 到 1 的进度采样缓动结果。
    ///
    /// 参数：
    /// - `progress`：原始线性进度。
    ///
    /// 返回值：
    /// - 返回应用当前曲线后的进度值。
    #[inline]
    pub fn sample(self, progress: f32) -> f32 {
        #[inline]
        fn cube(value: f32) -> f32 {
            value * value * value
        }

        let progress = progress.clamp(0.0, 1.0);
        match self {
            Self::Linear => progress,
            Self::EaseInCubic => cube(progress),
            Self::EaseOutCubic => 1.0 - cube(1.0 - progress),
            Self::EaseInOutCubic => {
                if progress < 0.5 {
                    4.0 * cube(progress)
                } else {
                    1.0 - (cube(-2.0 * progress + 2.0) / 2.0)
                }
            }
        }
    }
}

pub type Easing = AnimationCurve;

/// 控制动画在开始前和结束后的取值策略。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillMode {
    None,
    Forwards,
    Backwards,
    Both,
}

/// 控制动画每个周期的播放方向。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackDirection {
    Normal,
    Reverse,
    Alternate,
    AlternateReverse,
}

impl PlaybackDirection {
    pub(super) fn toggled(self) -> Self {
        match self {
            Self::Normal => Self::Reverse,
            Self::Reverse => Self::Normal,
            Self::Alternate => Self::AlternateReverse,
            Self::AlternateReverse => Self::Alternate,
        }
    }

    pub(super) fn starts_reversed(self) -> bool {
        matches!(self, Self::Reverse | Self::AlternateReverse)
    }
}

/// 控制动画重复次数。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeat {
    Count(u32),
    Infinite,
}

impl Repeat {
    pub(super) fn finite_cycles(self) -> Option<u32> {
        match self {
            Self::Count(count) => Some(count.max(1)),
            Self::Infinite => None,
        }
    }
}

/// 动画播放配置。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Playback {
    delay: Duration,
    repeat: Repeat,
    direction: PlaybackDirection,
    speed: f32,
    fill_mode: FillMode,
}

impl Playback {
    /// 创建默认播放配置。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置动画开始前的延迟。
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// 设置有限重复次数，最小为 1。
    pub fn repeat(mut self, repeat: u32) -> Self {
        self.repeat = Repeat::Count(repeat.max(1));
        self
    }

    /// 设置无限重复播放。
    pub fn repeat_forever(mut self) -> Self {
        self.repeat = Repeat::Infinite;
        self
    }

    /// 设置播放方向。
    pub fn direction(mut self, direction: PlaybackDirection) -> Self {
        self.direction = direction;
        self
    }

    /// 设置播放速度，最小为 0。
    pub fn speed(mut self, speed: f32) -> Self {
        self.speed = speed.max(0.0);
        self
    }

    /// 设置填充模式。
    pub fn fill_mode(mut self, fill_mode: FillMode) -> Self {
        self.fill_mode = fill_mode;
        self
    }

    pub fn delay_duration(self) -> Duration {
        self.delay
    }

    pub fn repeat_mode(self) -> Repeat {
        self.repeat
    }

    pub fn direction_mode(self) -> PlaybackDirection {
        self.direction
    }

    pub fn speed_factor(self) -> f32 {
        self.speed
    }

    pub fn fill(self) -> FillMode {
        self.fill_mode
    }
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            delay: Duration::ZERO,
            repeat: Repeat::Count(1),
            direction: PlaybackDirection::Normal,
            speed: 1.0,
            fill_mode: FillMode::Both,
        }
    }
}

/// 简单过渡动画配置。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    duration: Duration,
    curve: AnimationCurve,
    playback: Playback,
}

impl Transition {
    /// 创建线性过渡。
    pub fn linear(duration: Duration) -> Self {
        Self {
            duration,
            curve: AnimationCurve::Linear,
            playback: Playback::default(),
        }
    }

    /// 创建 cubic ease-in 过渡。
    pub fn ease_in(duration: Duration) -> Self {
        Self {
            duration,
            curve: AnimationCurve::EaseInCubic,
            playback: Playback::default(),
        }
    }

    /// 创建 cubic ease-out 过渡。
    pub fn ease_out(duration: Duration) -> Self {
        Self {
            duration,
            curve: AnimationCurve::EaseOutCubic,
            playback: Playback::default(),
        }
    }

    /// 创建 cubic ease-in-out 过渡。
    pub fn ease_in_out(duration: Duration) -> Self {
        Self {
            duration,
            curve: AnimationCurve::EaseInOutCubic,
            playback: Playback::default(),
        }
    }

    /// 设置缓动曲线。
    pub fn curve(mut self, curve: AnimationCurve) -> Self {
        self.curve = curve;
        self
    }

    /// 设置开始延迟。
    pub fn delay(mut self, delay: Duration) -> Self {
        self.playback = self.playback.delay(delay);
        self
    }

    /// 设置重复次数。
    pub fn repeat(mut self, repeat: u32) -> Self {
        self.playback = self.playback.repeat(repeat);
        self
    }

    /// 设置无限重复。
    pub fn repeat_forever(mut self) -> Self {
        self.playback = self.playback.repeat_forever();
        self
    }

    /// 设置播放方向。
    pub fn direction(mut self, direction: PlaybackDirection) -> Self {
        self.playback = self.playback.direction(direction);
        self
    }

    /// 设置播放速度。
    pub fn speed(mut self, speed: f32) -> Self {
        self.playback = self.playback.speed(speed);
        self
    }

    /// 设置填充模式。
    pub fn fill_mode(mut self, fill_mode: FillMode) -> Self {
        self.playback = self.playback.fill_mode(fill_mode);
        self
    }

    /// 使用完整播放配置覆盖当前配置。
    pub fn playback(mut self, playback: Playback) -> Self {
        self.playback = playback;
        self
    }

    pub(crate) fn duration(self) -> Duration {
        self.duration
    }

    pub(crate) fn curve_mode(self) -> AnimationCurve {
        self.curve
    }

    pub(crate) fn playback_mode(self) -> Playback {
        self.playback
    }
}

impl Default for Transition {
    fn default() -> Self {
        Self::ease_out(Duration::from_millis(DEFAULT_DURATION_MS))
    }
}

/// 表示单个关键帧。
#[derive(Clone, Debug, PartialEq)]
pub struct Keyframe<T> {
    offset: Duration,
    value: T,
}

impl<T> Keyframe<T> {
    /// 在指定时间点创建关键帧。
    pub fn at(offset: Duration, value: T) -> Self {
        Self { offset, value }
    }

    /// 返回关键帧值。
    pub fn value(&self) -> &T {
        &self.value
    }

    /// 返回关键帧时间偏移。
    pub fn offset(&self) -> Duration {
        self.offset
    }
}

/// 表示一组关键帧定义。
#[derive(Clone, Debug, PartialEq)]
pub struct Keyframes<T> {
    total_duration: Duration,
    frames: Vec<Keyframe<T>>,
    curve: AnimationCurve,
    frames_sorted_by_offset: bool,
}

impl<T> Keyframes<T> {
    /// 使用总时长创建关键帧集合。
    pub fn timed(total_duration: Duration) -> Self {
        Self {
            total_duration,
            frames: Vec::new(),
            curve: AnimationCurve::Linear,
            frames_sorted_by_offset: true,
        }
    }

    /// 使用百分比语义创建关键帧集合。
    pub fn percent(total_duration: Duration) -> Self {
        Self::timed(total_duration)
    }

    /// 添加一个绝对时间偏移关键帧，超过总时长时会截断到末尾。
    pub fn at(mut self, offset: Duration, value: T) -> Self {
        self.push_frame(offset.min(self.total_duration), value);
        self
    }

    /// 按百分比添加关键帧。
    pub fn at_percent(mut self, percent: f32, value: T) -> Self {
        let progress = percent.clamp(0.0, 1.0) as f64;
        let offset = Duration::from_secs_f64(self.total_duration.as_secs_f64() * progress);
        self.push_frame(offset, value);
        self
    }

    /// 设置关键帧之间的插值曲线。
    pub fn curve(mut self, curve: AnimationCurve) -> Self {
        self.curve = curve;
        self
    }

    pub fn total_duration(&self) -> Duration {
        self.total_duration
    }

    pub fn frames(&self) -> &[Keyframe<T>] {
        &self.frames
    }

    /// 转换为 `AnimationSpec`。
    pub fn into_spec(self) -> AnimationSpec<T> {
        self.into()
    }

    pub(super) fn curve_mode(&self) -> AnimationCurve {
        self.curve
    }

    pub(super) fn frames_are_sorted_by_offset(&self) -> bool {
        self.frames_sorted_by_offset
    }

    pub(super) fn sorted_frames(&self) -> Vec<&Keyframe<T>> {
        let mut frames = self.frames.iter().collect::<Vec<_>>();
        frames.sort_by_key(|frame| frame.offset);
        frames
    }

    fn push_frame(&mut self, offset: Duration, value: T) {
        if let Some(last) = self.frames.last() {
            self.frames_sorted_by_offset &= last.offset <= offset;
        }
        self.frames.push(Keyframe::at(offset, value));
    }
}

/// 表示完整的可播放动画描述。
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationSpec<T> {
    pub(super) keyframes: Keyframes<T>,
    playback: Playback,
}

impl<T> AnimationSpec<T> {
    /// 使用关键帧创建动画描述。
    pub fn new(keyframes: Keyframes<T>) -> Self {
        Self {
            keyframes,
            playback: Playback::default(),
        }
    }

    /// 设置完整播放配置。
    pub fn playback(mut self, playback: Playback) -> Self {
        self.playback = playback;
        self
    }

    /// 设置开始延迟。
    pub fn delay(mut self, delay: Duration) -> Self {
        self.playback = self.playback.delay(delay);
        self
    }

    /// 设置重复次数。
    pub fn repeat(mut self, repeat: u32) -> Self {
        self.playback = self.playback.repeat(repeat);
        self
    }

    /// 设置无限重复。
    pub fn repeat_forever(mut self) -> Self {
        self.playback = self.playback.repeat_forever();
        self
    }

    /// 设置播放方向。
    pub fn direction(mut self, direction: PlaybackDirection) -> Self {
        self.playback = self.playback.direction(direction);
        self
    }

    /// 设置播放速度。
    pub fn speed(mut self, speed: f32) -> Self {
        self.playback = self.playback.speed(speed);
        self
    }

    /// 设置填充模式。
    pub fn fill_mode(mut self, fill_mode: FillMode) -> Self {
        self.playback = self.playback.fill_mode(fill_mode);
        self
    }

    pub fn keyframes(&self) -> &Keyframes<T> {
        &self.keyframes
    }

    pub fn playback_config(&self) -> Playback {
        self.playback
    }
}

impl<T> From<Keyframes<T>> for AnimationSpec<T> {
    fn from(value: Keyframes<T>) -> Self {
        Self::new(value)
    }
}

impl<T> From<AnimationSpec<T>> for Transition {
    fn from(value: AnimationSpec<T>) -> Self {
        Self {
            duration: value.keyframes.total_duration(),
            curve: value.keyframes.curve_mode(),
            playback: value.playback,
        }
    }
}
