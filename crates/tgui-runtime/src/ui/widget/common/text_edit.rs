use crate::ui::unit::Dp;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TextEditState {
    pub cursor: usize,
    pub anchor: usize,
    pub composition: Option<CompositionState>,
    pub scroll_x: Dp,
    pub scroll_y: Dp,
    pub preferred_column_x: Option<f32>,
}

impl TextEditState {
    pub(crate) fn caret_at(text: &str) -> Self {
        let end = text.len();
        Self {
            cursor: end,
            anchor: end,
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        }
    }

    pub(crate) fn selection_range(&self) -> Option<(usize, usize)> {
        (self.cursor != self.anchor)
            .then_some((self.cursor.min(self.anchor), self.cursor.max(self.anchor)))
    }

    pub(crate) fn clamped_to(mut self, text: &str) -> Self {
        self.cursor = clamp_to_char_boundary(text, self.cursor);
        self.anchor = clamp_to_char_boundary(text, self.anchor);
        if let Some(composition) = &mut self.composition {
            composition.replace_range.0 = clamp_to_char_boundary(text, composition.replace_range.0);
            composition.replace_range.1 = clamp_to_char_boundary(text, composition.replace_range.1);
            if composition.replace_range.0 > composition.replace_range.1 {
                composition.replace_range =
                    (composition.replace_range.1, composition.replace_range.1);
            }
            if let Some((start, end)) = composition.cursor {
                let start = clamp_to_char_boundary(&composition.text, start);
                let end = clamp_to_char_boundary(&composition.text, end);
                composition.cursor = Some(if start <= end {
                    (start, end)
                } else {
                    (end, end)
                });
            }
        }
        self.scroll_x = self.scroll_x.max(Dp::ZERO);
        self.scroll_y = self.scroll_y.max(Dp::ZERO);
        if let Some(preferred_column_x) = self.preferred_column_x {
            self.preferred_column_x = preferred_column_x
                .is_finite()
                .then_some(preferred_column_x.max(0.0));
        }
        self
    }
}

fn clamp_to_char_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompositionState {
    pub replace_range: (usize, usize),
    pub text: String,
    pub cursor: Option<(usize, usize)>,
}

#[cfg(test)]
mod tests {
    use super::{CompositionState, TextEditState};
    use crate::ui::unit::Dp;

    #[test]
    fn input_edit_state_clamps_to_utf8_char_boundaries() {
        let text = "输入框示例输入框示例输入框示例";

        let state = TextEditState {
            cursor: 25,
            anchor: 29,
            composition: Some(CompositionState {
                replace_range: (25, 29),
                text: "提示".to_string(),
                cursor: Some((1, 4)),
            }),
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        }
        .clamped_to(text);

        assert_eq!(state.cursor, 24);
        assert_eq!(state.anchor, 27);
        assert_eq!(
            state
                .composition
                .as_ref()
                .map(|composition| composition.replace_range),
            Some((24, 27))
        );
        assert_eq!(
            state
                .composition
                .as_ref()
                .and_then(|composition| composition.cursor),
            Some((0, 3))
        );
    }
}
