//! Menu / ContextMenu 在 collect 阶段的浮层渲染。
//!
//! 渲染 label、separator、checked indicator、glyph/SVG icon、shortcut hint 与 submenu 箭头；
//! - 分隔线 (`Separator`) 用细矩形；
//! - 每个非分隔项一个 `HitInteraction::SelectOption`——复用 select 已有的
//!   点击 → on_select + on_open_change(false) 关菜单链路；
//! - 接外部点击与 Esc 自动关闭，`return_focus_to` 指回 trigger；

use std::sync::Arc;

use crate::foundation::color::Color;
use crate::media::{resolve_media_rect, ContentFit, MediaSource, RasterRequest};
use crate::text::font::TextFontRequest;
use crate::ui::layout::Insets;
use crate::ui::unit::Dp;
use crate::ui::widget::common::{
    ComputedScene, FocusScopeState, HitGeometry, HitInteraction, HitRegion, Rect, RenderPrimitive,
    TextPrimitive, TexturePrimitive,
};
use crate::ui::widget::menu::{
    ContextMenuDescriptor, MenuDescriptor, MenuIcon, MenuItemKind, MenuItemState,
};
use crate::ui::widget::overlay::{
    collect::emit_overlay, Alignment, Anchor, AnchorKey, FlipPolicy, Overlay, OverlayContent,
    OverlayId, OverlayLayer, Placement,
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
        if let Some(menu) = &self.menu {
            if resolved_menu_open(self.id, menu, context) && !menu.disabled.resolve() {
                let style = Box::new(self.resolved_menu_style(menu, context, visual));
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

        self.emit_context_menu_overlay_if_open(context, computed, visual);
    }

    fn emit_context_menu_overlay_if_open(
        &self,
        context: &mut CollectContext<'_, '_>,
        computed: &mut ComputedScene<VM>,
        visual: &CollectVisualState,
    ) {
        let Some(menu) = &self.context_menu else {
            return;
        };
        if menu.disabled.resolve() {
            return;
        }
        let Some(anchor) = context.context_menu_anchor_states.get(&self.id).copied() else {
            return;
        };
        let descriptor = context_menu_as_menu_descriptor(menu);
        let style = Box::new(self.resolved_menu_style(&descriptor, context, visual));
        emit_menu_layer(
            self,
            context,
            computed,
            visual,
            &descriptor,
            &style,
            Anchor::Point(anchor),
            Some(OverlayId::new(self.id.raw() ^ CONTEXT_MENU_OVERLAY_TAG)),
        );
    }

    fn resolved_menu_style(
        &self,
        menu: &MenuDescriptor<VM>,
        context: &CollectContext<'_, '_>,
        visual: &CollectVisualState,
    ) -> MenuStyle {
        let mut style = menu.resolved_style(&context.style_context);
        context
            .style_sheet
            .apply_menu(&mut style, &context.style_context, &self.visual);
        context.style_sheet.apply_menu_state(
            &mut style,
            &context.style_context,
            &self.visual,
            visual.widget_state,
        );
        style
    }
}

const CONTEXT_MENU_OVERLAY_TAG: u64 = 0x4354584D454E5531; // "CTXMENU1"

fn resolved_menu_open<VM>(
    id: crate::ui::widget::WidgetId,
    menu: &MenuDescriptor<VM>,
    context: &CollectContext<'_, '_>,
) -> bool {
    if let Some(open) = &menu.open {
        return open.resolve();
    }
    if let (Some(group), Some(index)) = (menu.menubar_group, menu.menubar_index) {
        return context
            .menubar_active_states
            .get(&group.raw())
            .copied()
            .flatten()
            == Some(index);
    }
    context.menu_open_states.get(&id).copied().unwrap_or(false)
}

fn context_menu_as_menu_descriptor<VM>(menu: &ContextMenuDescriptor<VM>) -> MenuDescriptor<VM> {
    MenuDescriptor {
        items: menu.items.clone(),
        open: Some(crate::ui::layout::Value::Static(true)),
        on_open_change: menu.on_open_change.clone(),
        placement: Placement::bottom().align(Alignment::Start),
        flip_policy: FlipPolicy::FlipAndShift,
        disabled: menu.disabled.clone(),
        style: menu.style.clone(),
        menubar_group: None,
        menubar_index: None,
        menubar_set_active: None,
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

    // 预扫一轮：决定是否需要"勾选列"和"快捷键列"以便对齐。
    let any_checkable = menu
        .items
        .iter()
        .any(|item| matches!(item.kind, MenuItemKind::Checkable));
    let any_submenu = menu
        .items
        .iter()
        .any(|item| matches!(item.kind, MenuItemKind::Submenu));
    let any_icon = menu.items.iter().any(|item| {
        matches!(
            item.icon.as_ref(),
            Some(MenuIcon::Glyph(_) | MenuIcon::Svg(_))
        )
    });
    let any_shortcut = menu.items.iter().any(|item| {
        item.shortcut_hint
            .as_ref()
            .map(|hint| !hint.resolve().is_empty())
            .unwrap_or(false)
            || item.shortcut.is_some()
    });

    let check_glyph = "\u{2713}"; // ✓
    let arrow_glyph = "\u{25B8}"; // ▸

    let check_col_w = if any_checkable {
        font_manager
            .measure_text_layout_wrapped(
                check_glyph,
                font_request.clone(),
                font_size,
                line_height,
                letter_spacing,
                f32::INFINITY,
            )
            .width
    } else {
        0.0
    };
    let arrow_col_w = if any_submenu {
        font_manager
            .measure_text_layout_wrapped(
                arrow_glyph,
                font_request.clone(),
                font_size,
                line_height,
                letter_spacing,
                f32::INFINITY,
            )
            .width
    } else {
        0.0
    };
    let check_col_w_dp = Dp::from(check_col_w);
    let arrow_col_w_dp = Dp::from(arrow_col_w);
    // 图标列占主题 item_icon_size，独立于具体 glyph 的实际宽度，保证对齐。
    let icon_col_w_dp = if any_icon {
        style.item_icon_size
    } else {
        Dp::ZERO
    };
    let icon_pad = if any_icon {
        style.item_icon_gap
    } else {
        Dp::ZERO
    };
    let check_pad = if any_checkable {
        style.item_icon_gap
    } else {
        Dp::ZERO
    };
    let arrow_pad = if any_submenu {
        style.item_icon_gap
    } else {
        Dp::ZERO
    };

    let mut max_text_width = Dp::ZERO;
    let mut max_shortcut_width = Dp::ZERO;
    let mut total_height = Dp::ZERO;
    let mut row_metrics = Vec::with_capacity(menu.items.len());
    for item in &menu.items {
        match item.kind {
            MenuItemKind::Separator => {
                let h = style.separator_height + style.item_padding.top + style.item_padding.bottom;
                row_metrics.push(RowMetrics::Separator { height: h });
                total_height += h;
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
                let shortcut_text = item
                    .shortcut_hint
                    .as_ref()
                    .map(|hint| hint.resolve())
                    .filter(|s| !s.is_empty())
                    .or_else(|| item.shortcut.as_ref().map(|chord| format_chord(chord)));
                let shortcut_w = if let Some(text) = shortcut_text.as_ref() {
                    let layout = font_manager.measure_text_layout_wrapped(
                        text,
                        font_request.clone(),
                        font_size,
                        line_height,
                        letter_spacing,
                        f32::INFINITY,
                    );
                    Dp::from(layout.width)
                } else {
                    Dp::ZERO
                };
                if shortcut_w > max_shortcut_width {
                    max_shortcut_width = shortcut_w;
                }
                row_metrics.push(RowMetrics::Item {
                    label,
                    text_h,
                    height: cell_h,
                    shortcut: shortcut_text,
                    shortcut_w,
                });
                total_height += cell_h;
            }
        }
    }
    total_height += style.padding.top + style.padding.bottom;

    let shortcut_pad = if any_shortcut {
        style.shortcut_gap
    } else {
        Dp::ZERO
    };
    let content_w = check_col_w_dp
        + check_pad
        + max_text_width
        + shortcut_pad
        + max_shortcut_width
        + arrow_pad
        + arrow_col_w_dp
        + content_horizontal_padding;
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
    let background = style.background.resolve();
    if background.a > 0 {
        primitives.push(crate::ui::widget::overlay::OverlayPrimitive::Shape(
            RenderPrimitive {
                rect: bg_rect,
                color: background,
                corner_radius: style.radius.resolve().get(),
                stroke_width: 0.0,
                clip_rect: None,
                clip_mask: None,
            },
        ));
    }
    let border = style.border.resolve();
    let border_width = style.border_width.resolve().get();
    if border.a > 0 && border_width > 0.0 {
        primitives.push(crate::ui::widget::overlay::OverlayPrimitive::Shape(
            RenderPrimitive {
                rect: bg_rect,
                color: border,
                corner_radius: style.radius.resolve().get(),
                stroke_width: border_width,
                clip_rect: None,
                clip_mask: None,
            },
        ));
    }

    let mut hits = Vec::with_capacity(menu.items.len());
    let menu_id = element.id;
    let on_open_change = menu.on_open_change.clone();
    // 收集子菜单候选：(parent_item_index, item_rect)。emit_overlay 后用它递归 emit 子菜单。
    let mut submenu_candidates: Vec<(usize, Rect)> = Vec::new();

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
                cursor_y += height;
            }
            RowMetrics::Item {
                label,
                text_h,
                height,
                shortcut,
                shortcut_w,
            } => {
                let item_rect = Rect::new(item_left, cursor_y, item_width, height);
                let disabled = menu.items[index].disabled.resolve();
                let is_checked = menu.items[index]
                    .checked
                    .as_ref()
                    .map(|c| c.resolve())
                    .unwrap_or(false);
                let mut widget_state = context.widget_states.get_select_option(menu_id, index);
                widget_state.disabled = disabled;
                widget_state.selected = is_checked;
                widget_state.checked = is_checked;
                widget_state.open = matches!(menu.items[index].kind, MenuItemKind::Submenu)
                    && widget_state.hovered
                    && !menu.items[index].submenu.is_empty();
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
                let row_label_color = style.item_foreground.resolve(widget_state).resolve();
                let resolved_font = font_manager.resolve_text(&label, font_request.clone());
                let label_baseline_y = cursor_y + ((height - text_h) * 0.5).max(Dp::ZERO);

                // 勾选列：仅当本项是 Checkable 且 checked = true 时画 ✓
                let mut cursor_x = item_left + style.item_padding.left;
                if any_checkable {
                    if is_checked {
                        let check_frame =
                            Rect::new(cursor_x, label_baseline_y, check_col_w_dp, text_h);
                        primitives.push(crate::ui::widget::overlay::OverlayPrimitive::Text(
                            TextPrimitive {
                                content: Arc::from(check_glyph.to_string()),
                                rich_spans: None,
                                frame: check_frame,
                                quad: None,
                                color: style.checked_indicator_color.resolve(),
                                force_color: false,
                                font_family: Some(Arc::from(resolved_font.primary_font.clone())),
                                font_size,
                                font_weight: style.text_style.weight,
                                line_height,
                                letter_spacing,
                                wrap: crate::ui::widget::CanvasTextWrap::None,
                                overflow: CanvasTextOverflow::Clip,
                                horizontal_align: CanvasTextHorizontalAlign::Start,
                                vertical_align: CanvasTextVerticalAlign::Start,
                                clip_rect: None,
                                clip_mask: None,
                            },
                        ));
                    }
                    cursor_x = cursor_x + check_col_w_dp + check_pad;
                }

                // 图标列：仅当 any_icon=true 时占位（无图标的项保留空白对齐）。
                if any_icon {
                    match menu.items[index].icon.as_ref() {
                        Some(MenuIcon::Glyph(glyph)) => {
                            let icon_frame =
                                Rect::new(cursor_x, label_baseline_y, icon_col_w_dp, text_h);
                            primitives.push(crate::ui::widget::overlay::OverlayPrimitive::Text(
                                TextPrimitive {
                                    content: Arc::from(glyph.to_string()),
                                    rich_spans: None,
                                    frame: icon_frame,
                                    quad: None,
                                    color: row_label_color,
                                    force_color: false,
                                    font_family: Some(Arc::from(
                                        resolved_font.primary_font.clone(),
                                    )),
                                    font_size,
                                    font_weight: style.text_style.weight,
                                    line_height,
                                    letter_spacing,
                                    wrap: crate::ui::widget::CanvasTextWrap::None,
                                    overflow: CanvasTextOverflow::Clip,
                                    horizontal_align: CanvasTextHorizontalAlign::Start,
                                    vertical_align: CanvasTextVerticalAlign::Start,
                                    clip_rect: None,
                                    clip_mask: None,
                                },
                            ));
                        }
                        Some(MenuIcon::Svg(bytes)) => {
                            let icon_size = icon_col_w_dp.min(height);
                            let icon_frame = Rect::new(
                                cursor_x,
                                cursor_y + ((height - icon_size) * 0.5).max(Dp::ZERO),
                                icon_size,
                                icon_size,
                            );
                            if let Some(primitive) = svg_menu_icon_primitive(
                                bytes,
                                icon_frame,
                                row_label_color.a as f32 / 255.0,
                                context,
                            ) {
                                primitives.push(primitive);
                            }
                        }
                        None => {}
                    }
                    cursor_x = cursor_x + icon_col_w_dp + icon_pad;
                }

                // 主 label
                let label_max_w = item_width
                    - (cursor_x - item_left)
                    - style.item_padding.right
                    - shortcut_pad
                    - max_shortcut_width
                    - arrow_pad
                    - arrow_col_w_dp;
                let label_frame = Rect::new(
                    cursor_x,
                    label_baseline_y,
                    label_max_w.max(Dp::ZERO),
                    text_h,
                );
                primitives.push(crate::ui::widget::overlay::OverlayPrimitive::Text(
                    TextPrimitive {
                        content: Arc::from(label),
                        rich_spans: None,
                        frame: label_frame,
                        quad: None,
                        color: row_label_color,
                        force_color: false,
                        font_family: Some(Arc::from(resolved_font.primary_font.clone())),
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

                // submenu 箭头：item_rect 右端往里 arrow_col_w
                if any_submenu && matches!(menu.items[index].kind, MenuItemKind::Submenu) {
                    let arrow_x =
                        item_left + item_width - style.item_padding.right - arrow_col_w_dp;
                    let arrow_frame = Rect::new(arrow_x, label_baseline_y, arrow_col_w_dp, text_h);
                    primitives.push(crate::ui::widget::overlay::OverlayPrimitive::Text(
                        TextPrimitive {
                            content: Arc::from(arrow_glyph.to_string()),
                            rich_spans: None,
                            frame: arrow_frame,
                            quad: None,
                            color: style.submenu_arrow_color.resolve(widget_state).resolve(),
                            force_color: false,
                            font_family: Some(Arc::from(resolved_font.primary_font.clone())),
                            font_size,
                            font_weight: style.text_style.weight,
                            line_height,
                            letter_spacing,
                            wrap: crate::ui::widget::CanvasTextWrap::None,
                            overflow: CanvasTextOverflow::Clip,
                            horizontal_align: CanvasTextHorizontalAlign::Start,
                            vertical_align: CanvasTextVerticalAlign::Start,
                            clip_rect: None,
                            clip_mask: None,
                        },
                    ));
                }

                // 快捷键提示：右对齐到 item_rect 右端（在 submenu 箭头左侧）
                if let Some(shortcut_text) = shortcut.as_ref() {
                    let shortcut_right_edge = item_left + item_width
                        - style.item_padding.right
                        - if any_submenu {
                            arrow_pad + arrow_col_w_dp
                        } else {
                            Dp::ZERO
                        };
                    let shortcut_x = shortcut_right_edge - shortcut_w;
                    let shortcut_frame =
                        Rect::new(shortcut_x, label_baseline_y, shortcut_w, text_h);
                    primitives.push(crate::ui::widget::overlay::OverlayPrimitive::Text(
                        TextPrimitive {
                            content: Arc::from(shortcut_text.clone()),
                            rich_spans: None,
                            frame: shortcut_frame,
                            quad: None,
                            color: style.shortcut_color.resolve(),
                            force_color: false,
                            font_family: Some(Arc::from(resolved_font.primary_font.clone())),
                            font_size,
                            font_weight: style.text_style.weight,
                            line_height,
                            letter_spacing,
                            wrap: crate::ui::widget::CanvasTextWrap::None,
                            overflow: CanvasTextOverflow::Clip,
                            horizontal_align: CanvasTextHorizontalAlign::Start,
                            vertical_align: CanvasTextVerticalAlign::Start,
                            clip_rect: None,
                            clip_mask: None,
                        },
                    ));
                }

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
                // 收子菜单候选：父项为 Submenu kind + widget_state.hovered + submenu 非空
                if !disabled
                    && matches!(menu.items[index].kind, MenuItemKind::Submenu)
                    && !menu.items[index].submenu.is_empty()
                    && widget_state.hovered
                {
                    submenu_candidates.push((index, item_rect));
                }
                cursor_y += height;
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
        options: FocusScopeOptions::new().trap(true),
        active: true,
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

    let solved = emit_overlay(
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

    // 递归 emit 子菜单：父 overlay 的解算位置 + 父项 item_rect 组成绝对锚点。
    if !submenu_candidates.is_empty() {
        let origin_x = Dp::from(solved.rect.x);
        let origin_y = Dp::from(solved.rect.y);
        let parent_overlay_id = overlay_id.0;
        for (sub_idx, item_rect) in submenu_candidates {
            let absolute_item_rect = Rect::new(
                origin_x + item_rect.x,
                origin_y + item_rect.y,
                item_rect.width,
                item_rect.height,
            );
            // 合成一个 submenu descriptor：items = 父项的 submenu；其它选项继承自父 menu，
            // 但 menubar_group / set_active 清空（submenu 不参与 MenuBar 切换）。
            let nested_descriptor = MenuDescriptor {
                items: menu.items[sub_idx].submenu.clone(),
                open: Some(crate::ui::layout::Value::Static(true)),
                on_open_change: on_open_change.clone(),
                placement: crate::ui::widget::overlay::Placement::right()
                    .align(crate::ui::widget::overlay::Alignment::Start),
                flip_policy: menu.flip_policy,
                disabled: menu.disabled.clone(),
                style: menu.style.clone(),
                menubar_group: None,
                menubar_index: None,
                menubar_set_active: None,
            };
            // 唯一 overlay_id：父 id 与 sub_idx 组合，保证嵌套层级互不冲突。
            let nested_overlay_id =
                crate::ui::widget::OverlayId::new(parent_overlay_id ^ ((sub_idx as u64 + 1) << 32));
            emit_menu_layer(
                element,
                context,
                computed,
                visual,
                &nested_descriptor,
                style,
                crate::ui::widget::overlay::Anchor::Rect(absolute_item_rect),
                Some(nested_overlay_id),
            );
        }
    }
}

enum RowMetrics {
    Separator {
        height: Dp,
    },
    Item {
        label: String,
        text_h: Dp,
        height: Dp,
        shortcut: Option<String>,
        shortcut_w: Dp,
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

fn svg_menu_icon_primitive(
    bytes: &std::sync::Arc<[u8]>,
    icon_frame: Rect,
    opacity: f32,
    context: &CollectContext<'_, '_>,
) -> Option<crate::ui::widget::overlay::OverlayPrimitive> {
    let source = MediaSource::bytes(bytes.clone());
    let metadata = context.media.image_snapshot(&source, None);
    let target_frame = resolve_media_rect(icon_frame, metadata.intrinsic_size, ContentFit::Contain);
    let raster_request = RasterRequest::from_frame(target_frame, context.units.scale_factor())?;
    let snapshot = context.media.image_snapshot(&source, Some(raster_request));
    let texture = snapshot.texture.as_ref()?;

    Some(crate::ui::widget::overlay::OverlayPrimitive::Texture(
        TexturePrimitive {
            texture: std::sync::Arc::clone(texture),
            frame: target_frame,
            quad: None,
            uv_rect: None,
            corner_radius: 0.0,
            opacity: opacity.clamp(0.0, 1.0),
            clip_rect: None,
            clip_mask: None,
        },
    ))
}

/// 把 `KeyChord` 渲染成人类可读的显示文本，例如 "Ctrl+N"、"Ctrl+Shift+Z"。
/// 当 `MenuItem::shortcut_hint()` 未显式设置但 `MenuItem::shortcut()` 有值时
/// 用作后备显示。
fn format_chord(chord: &crate::ui::widget::menu::KeyChord) -> String {
    use crate::platform::keyboard::ModifiersState;
    let mut parts: Vec<&'static str> = Vec::new();
    if chord.mods.contains(ModifiersState::CONTROL) {
        parts.push("Ctrl");
    }
    if chord.mods.contains(ModifiersState::ALT) {
        parts.push("Alt");
    }
    if chord.mods.contains(ModifiersState::SHIFT) {
        parts.push("Shift");
    }
    if chord.mods.contains(ModifiersState::META) {
        parts.push("Meta");
    }
    let key_label = match &chord.key {
        crate::ui::widget::menu::ChordKey::Code(code) => format!("{:?}", code)
            .trim_start_matches("Key")
            .trim_start_matches("Digit")
            .to_string(),
        crate::ui::widget::menu::ChordKey::Named(named) => format!("{:?}", named),
    };
    if parts.is_empty() {
        key_label
    } else {
        format!("{}+{}", parts.join("+"), key_label)
    }
}

#[allow(dead_code)]
fn _menu_item_kind_marker(_item: &MenuItemState<()>) {}

#[allow(dead_code)]
fn _color_unused() -> Color {
    Color::TRANSPARENT
}
