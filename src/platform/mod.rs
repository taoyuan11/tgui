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

#[cfg(all(target_os = "android", feature = "android"))]
pub mod android {
    pub mod activity {
        pub use winit_android::activity::*;
    }

    pub use winit_android::{
        ActiveEventLoopExtAndroid, EventLoopBuilderExtAndroid, EventLoopExtAndroid,
        WindowExtAndroid,
    };
}

#[cfg(all(target_env = "ohos", feature = "ohos"))]
pub mod ohos {
    pub use tgui_winit_ohos::{
        ActiveEventLoopExtOhos, EventLoopBuilderExtOhos, OhosApp, WindowExtOhos,
        export_ohos_winit_app,
    };
}

pub(crate) mod backend;
