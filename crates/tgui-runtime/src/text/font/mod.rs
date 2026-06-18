mod catalog;
mod layout;
mod manager;
mod platform;
#[cfg(test)]
mod tests;

pub use catalog::{FontWeight, TextFontRequest};

pub(crate) use catalog::FontCatalog;
pub(crate) use layout::build_layout_info_from_buffer;
pub(crate) use layout::TextLayoutInfo;
pub(crate) use manager::FontManager;
