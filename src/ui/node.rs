use std::cell::RefCell;
use std::rc::Rc;

use crate::graphics::TextStyle;

#[derive(Clone, Copy)]
pub struct ClickEvent {
    pub x: f32,
    pub y: f32,
}

pub(crate) type ClickHandler = Rc<RefCell<dyn FnMut(ClickEvent)>>;
type DynamicText = Rc<dyn Fn() -> String>;

pub trait View {
    fn into_node(self) -> Node;
}

#[derive(Clone)]
pub(crate) enum TextSource {
    Static(String),
    Dynamic(DynamicText),
}

impl TextSource {
    pub(crate) fn eval(&self) -> String {
        match self {
            Self::Static(s) => s.clone(),
            Self::Dynamic(f) => f(),
        }
    }
}

pub(crate) enum Node {
    Column(ColumnNode),
    Row(RowNode),
    Box(BoxNode),
    Button(ButtonNode),
    Text(TextNode),
}

pub(crate) struct ColumnNode {
    pub children: Vec<Node>,
    pub spacing: f32,
    pub centered: bool,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

pub(crate) struct RowNode {
    pub children: Vec<Node>,
    pub spacing: f32,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

pub(crate) struct BoxNode {
    pub children: Vec<Node>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

pub(crate) struct ButtonNode {
    pub label: TextNode,
    pub on_click: Option<ClickHandler>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

#[derive(Clone)]
pub(crate) struct TextNode {
    pub source: TextSource,
    pub style: TextStyle,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

pub struct Element {
    pub(crate) node: Node,
}

impl Element {
    pub(crate) fn from_node(node: Node) -> Self {
        Self { node }
    }

    pub fn spacing(mut self, spacing: u32) -> Self {
        match &mut self.node {
            Node::Column(column) => column.spacing = spacing as f32,
            Node::Row(row) => row.spacing = spacing as f32,
            _ => {}
        }
        self
    }

    pub fn center(mut self) -> Self {
        if let Node::Column(column) = &mut self.node {
            column.centered = true;
        }
        self
    }

    pub fn on_click<F>(mut self, callback: F) -> Self
    where
        F: FnMut(ClickEvent) + 'static,
    {
        if let Node::Button(button) = &mut self.node {
            button.on_click = Some(Rc::new(RefCell::new(callback)));
        }
        self
    }

    pub fn width(mut self, width: u32) -> Self {
        let width = Some(width as f32);
        match &mut self.node {
            Node::Column(column) => column.width = width,
            Node::Row(row) => row.width = width,
            Node::Box(b) => b.width = width,
            Node::Button(button) => button.width = width,
            Node::Text(text) => text.width = width,
        }
        self
    }

    pub fn height(mut self, height: u32) -> Self {
        let height = Some(height as f32);
        match &mut self.node {
            Node::Column(column) => column.height = height,
            Node::Row(row) => row.height = height,
            Node::Box(b) => b.height = height,
            Node::Button(button) => button.height = height,
            Node::Text(text) => text.height = height,
        }
        self
    }

    pub fn font_size(mut self, size: u32) -> Self {
        if let Node::Text(text) = &mut self.node {
            text.style.font_size = size as f32;
        }
        self
    }

    pub fn font(mut self, name: impl Into<String>) -> Self {
        if let Node::Text(text) = &mut self.node {
            text.style.font_name = Some(name.into());
        }
        self
    }

    pub fn color(mut self, color: [f32; 4]) -> Self {
        if let Node::Text(text) = &mut self.node {
            text.style.color = color;
        }
        self
    }

    pub fn color_rgb(mut self, r: u8, g: u8, b: u8) -> Self {
        if let Node::Text(text) = &mut self.node {
            text.style.color = [
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
                1.0,
            ];
        }
        self
    }

    pub fn color_rgba(mut self, r: u8, g: u8, b: u8, a: u8) -> Self {
        if let Node::Text(text) = &mut self.node {
            text.style.color = [
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
                a as f32 / 255.0,
            ];
        }
        self
    }

    pub fn letter_spacing(mut self, value: f32) -> Self {
        if let Node::Text(text) = &mut self.node {
            text.style.letter_spacing = value;
        }
        self
    }

    pub fn line_height(mut self, value: f32) -> Self {
        if let Node::Text(text) = &mut self.node {
            text.style.line_height = Some(value);
        }
        self
    }
}

impl View for Element {
    fn into_node(self) -> Node {
        self.node
    }
}

pub trait IntoTextSource {
    fn into_text_source(self) -> TextSource;
}

impl IntoTextSource for &str {
    fn into_text_source(self) -> TextSource {
        TextSource::Static(self.to_string())
    }
}

impl IntoTextSource for String {
    fn into_text_source(self) -> TextSource {
        TextSource::Static(self)
    }
}

impl<F> IntoTextSource for F
where
    F: Fn() -> String + 'static,
{
    fn into_text_source(self) -> TextSource {
        TextSource::Dynamic(Rc::new(self))
    }
}
