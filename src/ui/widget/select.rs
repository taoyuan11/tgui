use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::{Align, Insets, LayoutStyle, Value};

use super::common::{
    CursorStyle, InteractionHandlers, MediaEventHandlers, Point, SelectOptionState, VisualStyle,
    WidgetId, WidgetKind,
};
use super::container::{set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue};
use super::core::Element;
use super::style::{SelectStyle, StyleResolver};

pub struct Select<VM, K, V> {
    options: Vec<SelectOption<K, V>>,
    selected_key: Value<Option<K>>,
    placeholder: Value<String>,
    open: Option<Value<bool>>,
    disabled: Value<bool>,
    on_change: Option<ValueCommand<VM, (K, V)>>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    layout: LayoutStyle,
    visual: VisualStyle,
    interactions: InteractionHandlers<VM>,
    media_events: MediaEventHandlers<VM>,
    background: Option<Value<Color>>,
    style: Option<StyleResolver<SelectStyle>>,
}

macro_rules! impl_select_layout_api {
    () => {
        pub fn size(mut self, width: impl IntoLengthValue, height: impl IntoLengthValue) -> Self {
            set_layout_lengths(&mut self.layout, width, height);
            self
        }

        pub fn width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.width, width);
            self
        }

        pub fn height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.height, height);
            self
        }

        pub fn min_width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.min_width, width);
            self
        }

        pub fn min_height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.min_height, height);
            self
        }

        pub fn max_width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.max_width, width);
            self
        }

        pub fn max_height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.max_height, height);
            self
        }

        pub fn aspect_ratio(mut self, aspect_ratio: impl Into<Value<f32>>) -> Self {
            self.layout.aspect_ratio = Some(aspect_ratio.into());
            self
        }

        pub fn margin(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.layout.margin = insets.into();
            self
        }

        pub fn padding(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.layout.padding = Some(insets.into());
            self
        }

        pub fn grow(mut self, grow: impl Into<Value<f32>>) -> Self {
            self.layout.grow = grow.into();
            self
        }

        pub fn shrink(mut self, shrink: impl Into<Value<f32>>) -> Self {
            self.layout.shrink = shrink.into();
            self
        }

        pub fn basis(mut self, basis: impl IntoLengthValue) -> Self {
            self.layout.basis = Some(basis.into_length_value());
            self
        }

        pub fn align_self(mut self, align: Align) -> Self {
            self.layout.align_self = Some(align);
            self
        }

        pub fn justify_self(mut self, align: Align) -> Self {
            self.layout.justify_self = Some(align);
            self
        }

        pub fn column(mut self, start: usize) -> Self {
            self.layout.column_start = Some(start.max(1));
            self
        }

        pub fn row(mut self, start: usize) -> Self {
            self.layout.row_start = Some(start.max(1));
            self
        }

        pub fn column_span(mut self, span: usize) -> Self {
            self.layout.column_span = span.max(1);
            self
        }

        pub fn row_span(mut self, span: usize) -> Self {
            self.layout.row_span = span.max(1);
            self
        }

        pub fn position_absolute(mut self) -> Self {
            self.layout.position_type = crate::ui::layout::PositionType::Absolute;
            self
        }

        pub fn left(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.left, value);
            self
        }

        pub fn top(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.top, value);
            self
        }

        pub fn right(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.right, value);
            self
        }

        pub fn bottom(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.bottom, value);
            self
        }

        pub fn inset(mut self, value: impl IntoLengthValue + Copy) -> Self {
            set_layout_inset(&mut self.layout.left, value);
            set_layout_inset(&mut self.layout.top, value);
            set_layout_inset(&mut self.layout.right, value);
            set_layout_inset(&mut self.layout.bottom, value);
            self
        }
    };
}

impl<VM, K, V> Select<VM, K, V> {
    pub fn new<O>(options: Vec<O>, selected_key: impl Into<Value<Option<K>>>) -> Self
    where
        O: Into<SelectOption<K, V>>,
    {
        let mut interactions = InteractionHandlers::default();
        interactions.cursor_style = Some(Value::Static(CursorStyle::Pointer));

        Self {
            options: options.into_iter().map(Into::into).collect(),
            selected_key: selected_key.into(),
            placeholder: Value::Static(String::new()),
            open: None,
            disabled: Value::Static(false),
            on_change: None,
            on_open_change: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            interactions,
            media_events: MediaEventHandlers::default(),
            background: None,
            style: None,
        }
    }

    impl_select_layout_api!();

    pub fn placeholder(mut self, placeholder: impl Into<Value<String>>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn disable(mut self, disable: impl Into<Value<bool>>) -> Self {
        self.disabled = disable.into();
        self
    }

    pub fn open(mut self, open: impl Into<Value<bool>>) -> Self {
        self.open = Some(open.into());
        self
    }

    pub fn on_change(mut self, command: ValueCommand<VM, (K, V)>) -> Self {
        self.on_change = Some(command);
        self
    }

    pub fn on_open_change(mut self, command: ValueCommand<VM, bool>) -> Self {
        self.on_open_change = Some(command);
        self
    }

    pub fn style(
        mut self,
        resolver: impl Fn(ResolvedThemeMode) -> SelectStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::new(resolver));
        self
    }

    pub fn on_click(mut self, command: Command<VM>) -> Self {
        self.interactions.on_click = Some(command);
        self
    }

    pub fn on_double_click(mut self, command: Command<VM>) -> Self {
        self.interactions.on_double_click = Some(command);
        self
    }

    pub fn on_focus(mut self, command: Command<VM>) -> Self {
        self.interactions.on_focus = Some(command);
        self
    }

    pub fn on_blur(mut self, command: Command<VM>) -> Self {
        self.interactions.on_blur = Some(command);
        self
    }

    pub fn on_mouse_enter(mut self, command: Command<VM>) -> Self {
        self.interactions.on_mouse_enter = Some(command);
        self
    }

    pub fn on_mouse_leave(mut self, command: Command<VM>) -> Self {
        self.interactions.on_mouse_leave = Some(command);
        self
    }

    pub fn on_mouse_move(mut self, command: ValueCommand<VM, Point>) -> Self {
        self.interactions.on_mouse_move = Some(command);
        self
    }

    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.interactions.cursor_style = Some(cursor.into());
        self
    }
}

impl<VM, K, V> From<Select<VM, K, V>> for Element<VM>
where
    VM: 'static,
    K: Clone + PartialEq + Send + Sync + 'static,
    V: Clone + Into<Value<String>> + Send + Sync + 'static,
{
    fn from(select: Select<VM, K, V>) -> Self {
        let label_options = select
            .options
            .iter()
            .map(|option| {
                let label = option
                    .label
                    .clone()
                    .unwrap_or_else(|| option.value.clone().into());
                (option.key.clone(), label)
            })
            .collect::<Vec<_>>();
        let selected_label = select_selected_label(&select.selected_key, label_options.clone());
        let options = select
            .options
            .into_iter()
            .zip(label_options)
            .map(|(option, (key, label))| {
                let selected = select_option_selected(&select.selected_key, key);
                let on_select = select.on_change.clone().map(|command| {
                    let key = option.key.clone();
                    let value = option.value.clone();
                    Command::new_with_context(move |view_model: &mut VM, context| {
                        command.execute_with_context(
                            view_model,
                            (key.clone(), value.clone()),
                            context,
                        );
                    })
                });
                SelectOptionState {
                    label,
                    selected,
                    disabled: option.disabled,
                    on_select,
                }
            })
            .collect();

        Element {
            id: WidgetId::next(),
            layout: select.layout,
            visual: select.visual,
            interactions: select.interactions,
            media_events: select.media_events,
            background: select.background,
            kind: WidgetKind::Select {
                selected_label,
                placeholder: select.placeholder,
                options,
                open: select.open,
                on_open_change: select.on_open_change,
                disabled: select.disabled,
                style: select.style,
            },
        }
    }
}

#[derive(Clone)]
pub struct SelectOption<K, V> {
    key: K,
    value: V,
    label: Option<Value<String>>,
    disabled: Value<bool>,
}

impl<K, V> SelectOption<K, V> {
    pub fn new(key: K, value: V) -> Self {
        Self {
            key,
            value,
            label: None,
            disabled: Value::Static(false),
        }
    }

    pub fn label(mut self, label: impl Into<Value<String>>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn disable(mut self, disable: impl Into<Value<bool>>) -> Self {
        self.disabled = disable.into();
        self
    }
}

impl<K, V> From<(K, V)> for SelectOption<K, V> {
    fn from((key, value): (K, V)) -> Self {
        Self::new(key, value)
    }
}

fn select_option_selected<K>(selected_key: &Value<Option<K>>, option_key: K) -> Value<bool>
where
    K: Clone + PartialEq + Send + Sync + 'static,
{
    match selected_key {
        Value::Static(current) => Value::Static(current.as_ref() == Some(&option_key)),
        Value::Bound(binding) => binding
            .map(move |current| current.as_ref() == Some(&option_key))
            .into(),
    }
}

fn select_selected_label<K>(
    selected_key: &Value<Option<K>>,
    options: Vec<(K, Value<String>)>,
) -> Value<Option<String>>
where
    K: Clone + PartialEq + Send + Sync + 'static,
{
    match selected_key {
        Value::Static(current) => Value::Static(
            current
                .as_ref()
                .and_then(|key| selected_label_for_key(key, &options)),
        ),
        Value::Bound(binding) => binding
            .map(move |current| {
                current
                    .as_ref()
                    .and_then(|key| selected_label_for_key(key, &options))
            })
            .into(),
    }
}

fn selected_label_for_key<K>(key: &K, options: &[(K, Value<String>)]) -> Option<String>
where
    K: PartialEq,
{
    options
        .iter()
        .find(|(option_key, _)| option_key == key)
        .map(|(_, label)| label.resolve())
}
