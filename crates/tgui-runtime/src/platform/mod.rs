//! 平台抽象层，统一导出 winit 相关基础类型与后端封装。

pub mod dpi {
    pub use dpi::*;
}

pub mod error {
    pub use winit::error::{EventLoopError, ExternalError, NotSupportedError, OsError};

    /// Compatibility alias for the 0.31 split-crate request error name.
    pub type RequestError = ExternalError;
}

pub mod event {
    use std::path::PathBuf;

    use winit::dpi::PhysicalPosition;
    use winit::event::KeyEvent as WinitKeyEvent;
    pub use winit::event::{
        DeviceId, ElementState, Force, Ime, MouseButton, MouseScrollDelta, Touch, TouchPhase,
    };
    use winit::event::{TouchPhase as WinitTouchPhase, WindowEvent as WinitWindowEvent};
    use winit::keyboard::{Key, KeyLocation, ModifiersState, PhysicalKey, SmolStr};
    use winit::window::Theme;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct FingerId(u64);

    impl FingerId {
        pub const fn from_raw(value: u64) -> Self {
            Self(value)
        }

        pub const fn raw(self) -> u64 {
            self.0
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum PointerSource {
        Mouse,
        Touch {
            finger_id: FingerId,
            force: Option<Force>,
        },
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub enum PointerKind {
        Mouse,
        Touch(FingerId),
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum ButtonSource {
        Mouse(MouseButton),
        Touch {
            finger_id: FingerId,
            force: Option<Force>,
        },
    }

    impl ButtonSource {
        pub fn mouse_button(self) -> Option<MouseButton> {
            match self {
                Self::Mouse(button) => Some(button),
                // Touch presses map to primary activation for widgets that do not
                // distinguish physical mouse buttons from touch taps.
                Self::Touch { .. } => Some(MouseButton::Left),
            }
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq, Hash)]
    pub struct KeyEvent {
        pub physical_key: PhysicalKey,
        pub logical_key: Key,
        pub text: Option<SmolStr>,
        pub location: KeyLocation,
        pub state: ElementState,
        pub repeat: bool,
    }

    impl From<WinitKeyEvent> for KeyEvent {
        fn from(event: WinitKeyEvent) -> Self {
            Self {
                physical_key: event.physical_key,
                logical_key: event.logical_key,
                text: event.text,
                location: event.location,
                state: event.state,
                repeat: event.repeat,
            }
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    pub enum WindowEvent {
        SurfaceResized(winit::dpi::PhysicalSize<u32>),
        CloseRequested,
        Focused(bool),
        KeyboardInput {
            device_id: Option<DeviceId>,
            event: KeyEvent,
        },
        ModifiersChanged(winit::event::Modifiers),
        Ime(Ime),
        PointerMoved {
            device_id: Option<DeviceId>,
            position: PhysicalPosition<f64>,
            primary: bool,
            source: PointerSource,
        },
        PointerEntered {
            device_id: Option<DeviceId>,
            position: PhysicalPosition<f64>,
            primary: bool,
            kind: PointerKind,
        },
        PointerLeft {
            device_id: Option<DeviceId>,
            position: Option<PhysicalPosition<f64>>,
            primary: bool,
            kind: PointerKind,
        },
        MouseWheel {
            device_id: Option<DeviceId>,
            delta: MouseScrollDelta,
            phase: TouchPhase,
        },
        PointerButton {
            device_id: Option<DeviceId>,
            state: ElementState,
            position: PhysicalPosition<f64>,
            button: ButtonSource,
            primary: bool,
        },
        DragDropped {
            paths: Vec<PathBuf>,
            position: PhysicalPosition<f64>,
        },
        ThemeChanged(Theme),
        ScaleFactorChanged,
        RedrawRequested,
    }

    impl WindowEvent {
        pub(crate) fn from_winit(
            event: WinitWindowEvent,
            cursor_position: Option<PhysicalPosition<f64>>,
        ) -> Vec<Self> {
            match event {
                WinitWindowEvent::Resized(size) => vec![Self::SurfaceResized(size)],
                WinitWindowEvent::CloseRequested => vec![Self::CloseRequested],
                WinitWindowEvent::Focused(focused) => vec![Self::Focused(focused)],
                WinitWindowEvent::KeyboardInput {
                    device_id, event, ..
                } => {
                    vec![Self::KeyboardInput {
                        device_id: Some(device_id),
                        event: event.into(),
                    }]
                }
                WinitWindowEvent::ModifiersChanged(modifiers) => {
                    vec![Self::ModifiersChanged(modifiers)]
                }
                WinitWindowEvent::Ime(event) => vec![Self::Ime(event)],
                WinitWindowEvent::CursorMoved {
                    device_id,
                    position,
                } => vec![Self::PointerMoved {
                    device_id: Some(device_id),
                    position,
                    primary: true,
                    source: PointerSource::Mouse,
                }],
                WinitWindowEvent::CursorEntered { device_id } => vec![Self::PointerEntered {
                    device_id: Some(device_id),
                    position: cursor_position.unwrap_or_else(|| PhysicalPosition::new(0.0, 0.0)),
                    primary: true,
                    kind: PointerKind::Mouse,
                }],
                WinitWindowEvent::CursorLeft { device_id } => vec![Self::PointerLeft {
                    device_id: Some(device_id),
                    position: cursor_position,
                    primary: true,
                    kind: PointerKind::Mouse,
                }],
                WinitWindowEvent::MouseWheel {
                    device_id,
                    delta,
                    phase,
                } => {
                    vec![Self::MouseWheel {
                        device_id: Some(device_id),
                        delta,
                        phase,
                    }]
                }
                WinitWindowEvent::MouseInput {
                    device_id,
                    state,
                    button,
                } => {
                    let primary = button == MouseButton::Left;
                    vec![Self::PointerButton {
                        device_id: Some(device_id),
                        state,
                        position: cursor_position
                            .unwrap_or_else(|| PhysicalPosition::new(0.0, 0.0)),
                        button: ButtonSource::Mouse(button),
                        primary,
                    }]
                }
                WinitWindowEvent::Touch(touch) => {
                    let finger_id = FingerId::from_raw(touch.id);
                    let position = touch.location;
                    let force = touch.force;
                    match touch.phase {
                        WinitTouchPhase::Started => vec![
                            Self::PointerMoved {
                                device_id: Some(DeviceId::dummy()),
                                position,
                                primary: true,
                                source: PointerSource::Touch { finger_id, force },
                            },
                            Self::PointerButton {
                                device_id: Some(DeviceId::dummy()),
                                state: ElementState::Pressed,
                                position,
                                button: ButtonSource::Touch { finger_id, force },
                                primary: true,
                            },
                        ],
                        WinitTouchPhase::Moved => vec![Self::PointerMoved {
                            device_id: Some(DeviceId::dummy()),
                            position,
                            primary: true,
                            source: PointerSource::Touch { finger_id, force },
                        }],
                        WinitTouchPhase::Ended => vec![Self::PointerButton {
                            device_id: Some(DeviceId::dummy()),
                            state: ElementState::Released,
                            position,
                            button: ButtonSource::Touch { finger_id, force },
                            primary: true,
                        }],
                        WinitTouchPhase::Cancelled => vec![Self::PointerLeft {
                            device_id: Some(DeviceId::dummy()),
                            position: Some(position),
                            primary: true,
                            kind: PointerKind::Touch(finger_id),
                        }],
                    }
                }
                WinitWindowEvent::DroppedFile(path) => vec![Self::DragDropped {
                    paths: vec![path],
                    position: cursor_position.unwrap_or_else(|| PhysicalPosition::new(0.0, 0.0)),
                }],
                WinitWindowEvent::ThemeChanged(theme) => vec![Self::ThemeChanged(theme)],
                WinitWindowEvent::ScaleFactorChanged { .. } => vec![Self::ScaleFactorChanged],
                WinitWindowEvent::RedrawRequested => vec![Self::RedrawRequested],
                _ => Vec::new(),
            }
        }
    }

    pub fn empty_modifiers() -> ModifiersState {
        ModifiersState::empty()
    }
}

pub mod keyboard {
    pub use winit::keyboard::*;

    pub fn meta_modifier() -> ModifiersState {
        ModifiersState::SUPER
    }

    pub fn has_meta_modifier(modifiers: ModifiersState) -> bool {
        modifiers.super_key()
    }
}

pub mod cursor {
    pub use winit::window::{Cursor, CursorIcon};
}

pub mod window {
    use winit::dpi::{Position, Size};
    pub use winit::window::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ImeHint {
        NONE,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ImePurpose {
        Normal,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ImeSurroundingText {
        pub text: String,
        pub cursor: usize,
        pub anchor: usize,
    }

    impl ImeSurroundingText {
        pub fn new(text: String, cursor: usize, anchor: usize) -> Result<Self, ()> {
            if !text.is_char_boundary(cursor) || !text.is_char_boundary(anchor) {
                return Err(());
            }
            Ok(Self {
                text,
                cursor,
                anchor,
            })
        }
    }

    #[derive(Clone, Debug)]
    pub struct ImeRequestData {
        pub cursor_area: Option<(Position, Size)>,
        pub surrounding_text: Option<ImeSurroundingText>,
        pub hint_and_purpose: Option<(ImeHint, ImePurpose)>,
    }

    impl Default for ImeRequestData {
        fn default() -> Self {
            Self {
                cursor_area: None,
                surrounding_text: None,
                hint_and_purpose: None,
            }
        }
    }

    impl ImeRequestData {
        pub fn with_cursor_area(mut self, position: Position, size: Size) -> Self {
            self.cursor_area = Some((position, size));
            self
        }

        pub fn with_surrounding_text(mut self, surrounding_text: ImeSurroundingText) -> Self {
            self.surrounding_text = Some(surrounding_text);
            self
        }

        pub fn with_hint_and_purpose(mut self, hint: ImeHint, purpose: ImePurpose) -> Self {
            self.hint_and_purpose = Some((hint, purpose));
            self
        }
    }

    #[derive(Clone, Copy, Debug, Default)]
    pub struct ImeCapabilities {
        pub hint_and_purpose: bool,
        pub cursor_area: bool,
        pub surrounding_text: bool,
    }

    impl ImeCapabilities {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn with_hint_and_purpose(mut self) -> Self {
            self.hint_and_purpose = true;
            self
        }

        pub fn with_cursor_area(mut self) -> Self {
            self.cursor_area = true;
            self
        }

        pub fn with_surrounding_text(mut self) -> Self {
            self.surrounding_text = true;
            self
        }
    }

    #[derive(Clone, Debug)]
    pub struct ImeEnableRequest {
        pub data: ImeRequestData,
    }

    impl ImeEnableRequest {
        pub fn new(_capabilities: ImeCapabilities, data: ImeRequestData) -> Option<Self> {
            Some(Self { data })
        }
    }

    #[derive(Clone, Debug)]
    pub enum ImeRequest {
        Enable(ImeEnableRequest),
        Update(ImeRequestData),
        Disable,
    }
}

pub(crate) mod accessibility;
pub(crate) mod backend;
