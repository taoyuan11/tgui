#[derive(Clone)]
struct CanvasState {
    tx: f32,
    ty: f32,
    text_style: TextStyle,
}

#[derive(Clone)]
pub struct TextStyle {
    pub font_name: Option<String>,
    pub font_size: f32,
    pub color: [f32; 4],
    pub letter_spacing: f32,
    pub line_height: Option<f32>,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_name: None,
            font_size: 16.0,
            color: [1.0, 1.0, 1.0, 1.0],
            letter_spacing: 0.0,
            line_height: None,
        }
    }
}

pub(crate) enum DrawCommand {
    FillRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
    },
    FillText {
        x: f32,
        y: f32,
        text: String,
        style: TextStyle,
    },
}

pub(crate) struct Canvas2D {
    commands: Vec<DrawCommand>,
    state: CanvasState,
}

impl Canvas2D {
    pub(crate) fn new() -> Self {
        Self {
            commands: Vec::new(),
            state: CanvasState {
                tx: 0.0,
                ty: 0.0,
                text_style: TextStyle::default(),
            },
        }
    }

    pub(crate) fn set_fill_style(&mut self, color: [f32; 4]) {
        self.state.text_style.color = color;
    }

    pub(crate) fn set_text_style(&mut self, style: TextStyle) {
        self.state.text_style = style;
    }

    pub(crate) fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.commands.push(DrawCommand::FillRect {
            x: x + self.state.tx,
            y: y + self.state.ty,
            w,
            h,
            color: self.state.text_style.color,
        });
    }

    pub(crate) fn fill_text(&mut self, text: String, x: f32, y: f32) {
        self.commands.push(DrawCommand::FillText {
            x: x + self.state.tx,
            y: y + self.state.ty,
            text,
            style: self.state.text_style.clone(),
        });
    }

    pub(crate) fn into_commands(self) -> Vec<DrawCommand> {
        self.commands
    }
}
