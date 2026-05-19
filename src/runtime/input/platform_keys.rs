#[cfg(target_os = "windows")]
use crate::platform::keyboard::KeyCode;
use crate::platform::keyboard::PhysicalKey;

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_ADD, VK_BACK, VK_DECIMAL, VK_DELETE, VK_DIVIDE, VK_DOWN, VK_END, VK_HOME,
    VK_LEFT, VK_MULTIPLY, VK_NUMPAD0, VK_NUMPAD1, VK_NUMPAD2, VK_NUMPAD3, VK_NUMPAD4, VK_NUMPAD5,
    VK_NUMPAD6, VK_NUMPAD7, VK_NUMPAD8, VK_NUMPAD9, VK_OEM_1, VK_OEM_102, VK_OEM_2, VK_OEM_3,
    VK_OEM_4, VK_OEM_5, VK_OEM_6, VK_OEM_7, VK_OEM_COMMA, VK_OEM_MINUS, VK_OEM_PERIOD, VK_OEM_PLUS,
    VK_RETURN, VK_RIGHT, VK_SEPARATOR, VK_SPACE, VK_SUBTRACT, VK_UP,
};

#[cfg(not(target_os = "windows"))]
pub(super) fn is_key_physically_pressed(_physical_key: PhysicalKey) -> bool {
    true
}

#[cfg(target_os = "windows")]
pub(super) fn is_key_physically_pressed(physical_key: PhysicalKey) -> bool {
    let Some(virtual_key) = physical_key_to_windows_virtual_key(physical_key) else {
        return true;
    };
    // SAFETY: `GetAsyncKeyState` 是无副作用的 Win32 调用，可以从任意线程查询全局
    // 键盘异步状态；`virtual_key` 已经被映射成合法的 `VK_*` 常量，转 i32 后落在
    // Win32 文档允许的取值区间，因此调用是安全的。
    unsafe { (GetAsyncKeyState(i32::from(virtual_key)) as u16 & 0x8000) != 0 }
}

#[cfg(target_os = "windows")]
fn physical_key_to_windows_virtual_key(physical_key: PhysicalKey) -> Option<u16> {
    let code = match physical_key {
        PhysicalKey::Code(KeyCode::Backspace) => VK_BACK.0,
        PhysicalKey::Code(KeyCode::Delete) => VK_DELETE.0,
        PhysicalKey::Code(KeyCode::ArrowLeft) => VK_LEFT.0,
        PhysicalKey::Code(KeyCode::ArrowRight) => VK_RIGHT.0,
        PhysicalKey::Code(KeyCode::ArrowUp) => VK_UP.0,
        PhysicalKey::Code(KeyCode::ArrowDown) => VK_DOWN.0,
        PhysicalKey::Code(KeyCode::Home) => VK_HOME.0,
        PhysicalKey::Code(KeyCode::End) => VK_END.0,
        PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => VK_RETURN.0,
        PhysicalKey::Code(KeyCode::Space) => VK_SPACE.0,
        PhysicalKey::Code(KeyCode::Numpad0) => VK_NUMPAD0.0,
        PhysicalKey::Code(KeyCode::Numpad1) => VK_NUMPAD1.0,
        PhysicalKey::Code(KeyCode::Numpad2) => VK_NUMPAD2.0,
        PhysicalKey::Code(KeyCode::Numpad3) => VK_NUMPAD3.0,
        PhysicalKey::Code(KeyCode::Numpad4) => VK_NUMPAD4.0,
        PhysicalKey::Code(KeyCode::Numpad5) => VK_NUMPAD5.0,
        PhysicalKey::Code(KeyCode::Numpad6) => VK_NUMPAD6.0,
        PhysicalKey::Code(KeyCode::Numpad7) => VK_NUMPAD7.0,
        PhysicalKey::Code(KeyCode::Numpad8) => VK_NUMPAD8.0,
        PhysicalKey::Code(KeyCode::Numpad9) => VK_NUMPAD9.0,
        PhysicalKey::Code(KeyCode::NumpadAdd) => VK_ADD.0,
        PhysicalKey::Code(KeyCode::NumpadSubtract) => VK_SUBTRACT.0,
        PhysicalKey::Code(KeyCode::NumpadDecimal) => VK_DECIMAL.0,
        PhysicalKey::Code(KeyCode::NumpadMultiply) => VK_MULTIPLY.0,
        PhysicalKey::Code(KeyCode::NumpadDivide) => VK_DIVIDE.0,
        PhysicalKey::Code(KeyCode::NumpadComma) => VK_SEPARATOR.0,
        PhysicalKey::Code(KeyCode::Minus) => VK_OEM_MINUS.0,
        PhysicalKey::Code(KeyCode::Equal) => VK_OEM_PLUS.0,
        PhysicalKey::Code(KeyCode::BracketLeft) => VK_OEM_4.0,
        PhysicalKey::Code(KeyCode::BracketRight) => VK_OEM_6.0,
        PhysicalKey::Code(KeyCode::Backslash) => VK_OEM_5.0,
        PhysicalKey::Code(KeyCode::IntlBackslash) => VK_OEM_102.0,
        PhysicalKey::Code(KeyCode::Semicolon) => VK_OEM_1.0,
        PhysicalKey::Code(KeyCode::Quote) => VK_OEM_7.0,
        PhysicalKey::Code(KeyCode::Backquote) => VK_OEM_3.0,
        PhysicalKey::Code(KeyCode::Comma) => VK_OEM_COMMA.0,
        PhysicalKey::Code(KeyCode::Period) => VK_OEM_PERIOD.0,
        PhysicalKey::Code(KeyCode::Slash) => VK_OEM_2.0,
        PhysicalKey::Code(KeyCode::Digit0) => b'0' as u16,
        PhysicalKey::Code(KeyCode::Digit1) => b'1' as u16,
        PhysicalKey::Code(KeyCode::Digit2) => b'2' as u16,
        PhysicalKey::Code(KeyCode::Digit3) => b'3' as u16,
        PhysicalKey::Code(KeyCode::Digit4) => b'4' as u16,
        PhysicalKey::Code(KeyCode::Digit5) => b'5' as u16,
        PhysicalKey::Code(KeyCode::Digit6) => b'6' as u16,
        PhysicalKey::Code(KeyCode::Digit7) => b'7' as u16,
        PhysicalKey::Code(KeyCode::Digit8) => b'8' as u16,
        PhysicalKey::Code(KeyCode::Digit9) => b'9' as u16,
        PhysicalKey::Code(KeyCode::KeyA) => b'A' as u16,
        PhysicalKey::Code(KeyCode::KeyB) => b'B' as u16,
        PhysicalKey::Code(KeyCode::KeyC) => b'C' as u16,
        PhysicalKey::Code(KeyCode::KeyD) => b'D' as u16,
        PhysicalKey::Code(KeyCode::KeyE) => b'E' as u16,
        PhysicalKey::Code(KeyCode::KeyF) => b'F' as u16,
        PhysicalKey::Code(KeyCode::KeyG) => b'G' as u16,
        PhysicalKey::Code(KeyCode::KeyH) => b'H' as u16,
        PhysicalKey::Code(KeyCode::KeyI) => b'I' as u16,
        PhysicalKey::Code(KeyCode::KeyJ) => b'J' as u16,
        PhysicalKey::Code(KeyCode::KeyK) => b'K' as u16,
        PhysicalKey::Code(KeyCode::KeyL) => b'L' as u16,
        PhysicalKey::Code(KeyCode::KeyM) => b'M' as u16,
        PhysicalKey::Code(KeyCode::KeyN) => b'N' as u16,
        PhysicalKey::Code(KeyCode::KeyO) => b'O' as u16,
        PhysicalKey::Code(KeyCode::KeyP) => b'P' as u16,
        PhysicalKey::Code(KeyCode::KeyQ) => b'Q' as u16,
        PhysicalKey::Code(KeyCode::KeyR) => b'R' as u16,
        PhysicalKey::Code(KeyCode::KeyS) => b'S' as u16,
        PhysicalKey::Code(KeyCode::KeyT) => b'T' as u16,
        PhysicalKey::Code(KeyCode::KeyU) => b'U' as u16,
        PhysicalKey::Code(KeyCode::KeyV) => b'V' as u16,
        PhysicalKey::Code(KeyCode::KeyW) => b'W' as u16,
        PhysicalKey::Code(KeyCode::KeyX) => b'X' as u16,
        PhysicalKey::Code(KeyCode::KeyY) => b'Y' as u16,
        PhysicalKey::Code(KeyCode::KeyZ) => b'Z' as u16,
        _ => return None,
    };
    Some(code)
}
