//! Menu / ContextMenu 在 collect 阶段的浮层渲染。
//!
//! step 3 范围：
//! - 仅渲染 label（暂不画 icon / shortcut hint / checked indicator / submenu 箭头）；
//! - 分隔线 (`Separator`) 用细矩形；
//! - 每个非分隔项一个 `HitInteraction::SelectOption`——复用 select 已有的
//!   点击 → on_select + on_open_change(false) 关菜单链路；
//! - 接外部点击与 Esc 自动关闭，`return_focus_to` 指回 trigger；
//! - 不画 hover / pressed 态背景（由 runtime widget_states 标记，step 5+ 拼）。

use crate::foundation::color::Color;
use crate::text::font::TextFontRequest;
use crate::ui::layout::Insets;
use crate::ui::theme::WidgetState;
use crate::ui::unit::Dp;
use crate::ui::widget::common::{
    ComputedScene, FocusScopeState, HitGeometry, HitInteraction, HitRegion, Rect, RenderPrimitive,
    TextPrimitive,
};
use crate::ui::widget::menu::{MenuDescriptor, MenuItemKind, MenuItemState};
use crate::ui::widget::overlay::{
    collect::emit_overlay, Anchor, AnchorKey, Overlay, OverlayContent, OverlayId, OverlayLayer,
};
use crate::ui::widget::style::MenuStyle;
use crate::ui::widget::{CanvasTextHorizontalAlign, CanvasTextOverflow, CanvasTextVerticalAlign};
use crate::ui::widget::{FocusScopeOptions, OverlayPlacementOptions};

use super::super::scene::CollectContext;
use super::super::types::ResolvedElement;
use super::CollectVisualState;

impl<VM> ResolvedElement<VM> {
    pub(super) fn emit_menu_overlay_if_open(
        &self,
        context: &mut CollectContext<'_, '_>,
        computed: &mut ComputedScene<VM>,
        visual: &CollectVisualState,
    ) {
        let Some(menu) = &self.menu else {
            return;
        };
        if !menu.open.resolve() {
            return;
        }
        if menu.disabled.resolve() {
            return;
        }
        let theme_mode = crate::ui::widget::style::infer_theme_mode(context.theme);
        let style = menu.resolved_style(theme_mode);
        emit_menu_layer(
            self,
            context,
            computed,
            visual,
            menu,
            &style,
            Anchor::Key(AnchorKey::widget(self.id)),
            None,
        );
    }
}

/// 不被 Menu 修饰符限定的子流程——也供未来 ContextMenu / MenuBar 复用。
pub(crate) fn emit_menu_layer<VM>(
    element: &ResolvedElement<VM>,
    context: &mut CollectContext<'_, '_>,
    computed: &mut ComputedScene<VM>,
    visual: &CollectVisualState,
    menu: &MenuDescriptor<VM>,
    style: &MenuStyle,
    anchor: Anchor,
    overlay_id_override: Option<OverlayId>,
) {
    if menu.items.is_empty() {
        return;
    }

    let font_manager = context.font_manager;
    let units = context.units;
    let font_size = units.resolve_sp(style.text_style.size).max(1.0);
    let line_height = style
        .text_style
        .line_height
        .map(|h| units.resolve_sp(h))
        .unwrap_or(font_size * 1.4);
    let letter_spacing = style
        .text_style
        .letter_spacing
        .map(|s| units.resolve_sp(s))
        .unwrap_or(0.0);
    let font_request = TextFontRequest {
        preferred_font: style.text_style.font_family.as_deref(),
        weight: style.text_style.weight,
    };

    let content_horizontal_padding = style.item_padding.left + style.item_padding.right;
    let content_vertical_padding = style.item_padding.top + style.item_padding.bottom;
    let max_label_width = (style.max_width - content_horizontal_padding).max(Dp::ZERO);

    let mut max_text_width = Dp::ZERO;
    let mut total_height = Dp::ZERO;
    let mut row_metrics = Vec::with_capacity(menu.items.len());
    for item in &menu.items {
        match item.kind {
            MenuItemKind::Separator => {
                let h = style.separator_height + style.item_padding.top + style.item_padding.bottom;
                row_metrics.push(RowMetrics::Separator { height: h });
                total_height = total_height + h;
            }
            MenuItemKind::Action | MenuItemKind::Checkable | MenuItemKind::Submenu => {
                let label = item.label.as_ref().map(|l| l.resolve()).unwrap_or_default();
                let layout = font_manager.measure_text_layout_wrapped(
                    &label,
                    font_request.clone(),
                    font_size,
                    line_height,
                    letter_spacing,
                    max_label_width.get(),
                );
                let text_w = Dp::from(layout.width);
                let text_h = Dp::from(layout.height).max(Dp::from(line_height));
                let cell_h = (text_h + content_vertical_padding).max(style.item_min_height);
                if text_w > max_text_width {
                    max_text_width = text_w;
                }
                row_metrics.push(RowMetrics::Item {
                    label,
                    text_w,
                    text_h,
                    height: cell_h,
                });
                total_height = total_height + cell_h;
            }
        }
    }
    total_height = total_height + style.padding.top + style.padding.bottom;

    let content_w = max_text_width + content_horizontal_padding;
    let overlay_w = content_w.max(style.min_width).min(style.max_width)
        + style.padding.left
        + style.padding.right;
    let overlay_h = total_height;

    let placement_options = OverlayPlacementOptions {
        placement: menu.placement,
        offset: Dp::ZERO,
        cross_offset: Dp::ZERO,
        flip: menu.flip_policy,
        viewport_padding: insets_max(style.padding),
        clamp_to_viewport: true,
        match_anchor_width: false,
    };
    let _ = placement_options; // solved by emit_overlay internally

    // Background container shape: occupies the full overlay rect.
    let bg_rect = Rect::new(Dp::ZERO, Dp::ZERO, overlay_w, overlay_h);
    let mut primitives = Vec::new();
    primitives.push(crate::ui::widget::overlay::OverlayPrimitive::Shape(
        RenderPrimitive {
            rect: bg_rect,
            color: style.background.resolve(),
            corner_radius: style.radius.resolve().get(),
            stroke_width: style.border_width.resolve().get(),
            clip_rect: None,
            clip_mask: None,
        },
    ));

    let mut hits = Vec::with_capacity(menu.items.len());
    let menu_id = element.id;
    let on_open_change = menu.on_open_change.clone();

    let mut cursor_y = style.padding.top;
    let item_left = style.padding.left;
    let item_width = overlay_w - style.padding.left - style.padding.right;
    for (index, row) in row_metrics.into_iter().enumerate() {
        match row {
            RowMetrics::Separator { height } => {
                let line_y = cursor_y + style.item_padding.top + style.separator_height * 0.5;
                let line_rect = Rect::new(
                    item_left + style.separator_inset_x,
                    line_y,
                    item_width - style.separator_inset_x * 2.0,
                    style.separator_height,
                );
                primitives.push(crate::ui::widget::overlay::OverlayPrimitive::Shape(
                    RenderPrimitive {
                        rect: line_rect,
                        color: style.separator_color.resolve(),
                        corner_radius: 0.0,
                        stroke_width: 0.0,
                        clip_rect: None,
                        clip_mask: None,
                    },
                ));
                cursor_y = cursor_y + height;
            }
            RowMetrics::Item {
                label,
                text_w: _,
                text_h,
                height,
            } => {
                let item_rect = Rect::new(item_left, cursor_y, item_width, height);
                let disabled = menu.items[index].disabled.resolve();
                let widget_state = WidgetState {
                    disabled,
                    ..Default::default()
                };
                let item_bg = style.item_background.resolve(widget_state).resolve();
                if item_bg.a > 0 {
                    primitives.push(crate::ui::widget::overlay::OverlayPrimitive::Shape(
                        RenderPrimitive {
                            rect: item_rect,
                            color: item_bg,
                            corner_radius: 0.0,
                            stroke_width: 0.0,
                            clip_rect: None,
                            clip_mask: None,
                        },
                    ));
                }
                let label_origin_y = cursor_y + ((height - text_h) * 0.5).max(Dp::ZERO);
                let label_frame = Rect::new(
                    item_left + style.item_padding.left,
                    label_origin_y,
                    item_width - style.item_padding.left - style.item_padding.right,
                    text_h,
                );
                let label_color = style.item_foreground.resolve(widget_state).resolve();
                let resolved_font = font_manager.resolve_text(&label, font_request.clone());
                primitives.push(crate::ui::widget::overlay::OverlayPrimitive::Text(
                    TextPrimitive {
                        content: label,
                        rich_spans: None,
                        frame: label_frame,
                        quad: None,
                        color: label_color,
                        force_color: false,
                        font_family: Some(resolved_font.primary_font),
                        font_size,
                        font_weight: style.text_style.weight,
                        line_height,
                        letter_spacing,
                        wrap: crate::ui::widget::CanvasTextWrap::Word,
                        overflow: CanvasTextOverflow::Ellipsis,
                        horizontal_align: CanvasTextHorizontalAlign::Start,
                        vertical_align: CanvasTextVerticalAlign::Start,
                        clip_rect: None,
                        clip_mask: None,
                    },
                ));

                let on_select = menu.items[index].on_select.clone();
                hits.push(HitRegion {
                    rect: item_rect,
                    clip_rect: None,
                    geometry: HitGeometry::Rect,
                    scope_path: context.focus_scope_path(),
                    focus: None,
                    interaction: if disabled {
                        HitInteraction::Disabled { id: menu_id }
                    } else {
                        HitInteraction::SelectOption {
                            id: menu_id,
                            option_index: index,
                            interactions: Default::default(),
                            on_select,
                            on_open_change: on_open_change.clone(),
                        }
                    },
                });
                cursor_y = cursor_y + height;
            }
        }
    }

    let overlay_id = overlay_id_override.unwrap_or(OverlayId::new(menu_id.raw()));
    computed.register_widget_overlay_anchor(menu_id, visual.frame);
    let focus_scope = FocusScopeState {
        scope_id: menu_id,
        path: {
            let mut path = context.focus_scope_path();
            path.push(menu_id);
            path
        },
        options: FocusScopeOptions { trap: true },
    };
    let mut overlay = Overlay::<VM>::new(overlay_id, anchor)
        .source_widget(menu_id)
        .placement(menu.placement)
        .flip_policy(menu.flip_policy)
        .viewport_padding(insets_max(style.padding))
        .layer(OverlayLayer::Menu)
        .close_on_outside_click(true)
        .close_on_escape(true)
        .return_focus_to(menu_id)
        .focus_scope(focus_scope);
    if let Some(cmd) = on_open_change.clone() {
        overlay = overlay.on_close(cmd);
    }

    let _ = emit_overlay(
        computed,
        context.viewport,
        overlay,
        (overlay_w, overlay_h),
        OverlayContent::Batch {
            primitives,
            hits,
            clip_rect: None,
        },
    );
}

enum RowMetrics {
    Separator {
        height: Dp,
    },
    Item {
        label: String,
        text_w: Dp,
        text_h: Dp,
        height: Dp,
    },
}

fn insets_max(insets: Insets) -> Dp {
    let mut m = insets.left;
    if insets.top > m {
        m = insets.top;
    }
    if insets.right > m {
        m = insets.right;
    }
    if insets.bottom > m {
        m = insets.bottom;
    }
    m
}

#[allow(dead_code)]
fn _menu_item_kind_marker(_item: &MenuItemState<()>) {}

#[allow(dead_code)]
fn _color_unused() -> Color {
    Color::TRANSPARENT
}
