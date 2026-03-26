#![allow(private_interfaces)]

mod graphics;
mod font;
mod reactive;
mod runtime;
mod ui;

pub use font::{load_font, set_default_font};
pub use reactive::{Signal, create_signal};
pub use runtime::run;
pub use ui::{ClickEvent, Element, View, box_layout, button, column, row, text};

pub mod prelude {
    pub use crate::{
        View, box_layout, button, column, create_signal, load_font, row, run, text,
        set_default_font, r#box,
    };
}

pub use ui::r#box;
