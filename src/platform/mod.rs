//! 平台抽象层，统一导出 winit 相关基础类型与后端封装。

pub mod dpi {
    pub use dpi::*;
}

pub mod error {
    pub use winit_core::error::*;
}

pub mod event {
    pub use winit_core::event::*;
}

pub mod keyboard {
    pub use winit_core::keyboard::*;
}

pub mod cursor {
    pub use winit_core::cursor::*;
}

pub mod window {
    pub use winit_core::window::*;
}

pub(crate) mod backend;
