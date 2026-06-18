use crate::platform::event::{ElementState, MouseButton, WindowEvent};
use crate::platform::keyboard::{KeyCode, PhysicalKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputTrigger {
    MousePressed(MouseButton),
    MouseReleased(MouseButton),
    KeyPressed(KeyCode),
    KeyReleased(KeyCode),
}

impl InputTrigger {
    pub(crate) fn matches(self, event: &WindowEvent) -> bool {
        match (self, event) {
            (
                Self::MousePressed(expected_button),
                WindowEvent::PointerButton {
                    state: ElementState::Pressed,
                    button,
                    ..
                },
            ) => button.clone().mouse_button() == Some(expected_button),
            (
                Self::MouseReleased(expected_button),
                WindowEvent::PointerButton {
                    state: ElementState::Released,
                    button,
                    ..
                },
            ) => button.clone().mouse_button() == Some(expected_button),
            (Self::KeyPressed(expected_code), WindowEvent::KeyboardInput { event, .. }) => {
                event.state == ElementState::Pressed
                    && matches!(event.physical_key, PhysicalKey::Code(code) if code == expected_code)
            }
            (Self::KeyReleased(expected_code), WindowEvent::KeyboardInput { event, .. }) => {
                event.state == ElementState::Released
                    && matches!(event.physical_key, PhysicalKey::Code(code) if code == expected_code)
            }
            _ => false,
        }
    }
}
