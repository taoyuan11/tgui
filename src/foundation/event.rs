use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

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
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button,
                    ..
                },
            ) => *button == expected_button,
            (
                Self::MouseReleased(expected_button),
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    button,
                    ..
                },
            ) => *button == expected_button,
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
