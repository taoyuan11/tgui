pub(super) use super::*;

use crate::foundation::view_model::ValueCommand;
use crate::ui::widget::{Modal, ModalAction, WidgetId};

#[test]
fn modal_builder_attaches_descriptor() {
    let element: Element<()> = Modal::new(true)
        .title("Hello")
        .action(ModalAction::primary("OK"))
        .into();
    assert!(
        element.modal.is_some(),
        "modal descriptor must be attached to outer Stack element"
    );
    let descriptor = element.modal.as_ref().unwrap();
    assert!(descriptor.open.resolve(), "open should resolve to true");
    assert!(descriptor.close_on_escape);
    assert!(descriptor.close_on_backdrop_click);
}

#[test]
fn modal_closed_renders_minimal() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Modal::new(false)
            .title("Closed Title")
            .content(Text::new("Hidden content"))
            .action(ModalAction::primary("OK")),
    );
    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 600.0, 400.0),
        None,
        None,
        None,
        None,
        false,
    );

    // 关闭状态下，主场景文字应不包含 modal 内的 title / content / action 文本
    // （opacity=0 时这些 widget 仍可能进入 scene 但 alpha=0；用 overlay_texts 判断也可，
    //  我们只要求 close handler 未注册，且 overlay 层无渲染）。
    assert!(
        rendered.primitives.overlay_texts.is_empty(),
        "closed modal should not emit overlay texts, got {:?}",
        rendered
            .primitives
            .overlay_texts
            .iter()
            .map(|t| t.content.as_ref())
            .collect::<Vec<_>>()
    );
}

#[test]
fn modal_open_renders_title_and_action_labels() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Modal::new(true)
            .title("Confirm")
            .content(Text::new("Are you sure?"))
            .action(ModalAction::new("Cancel"))
            .action(ModalAction::primary("OK")),
    );
    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 600.0, 400.0),
        None,
        None,
        None,
        None,
        false,
    );

    // Modal 是 in-tree 渲染，所以 title / content / action 都进入主 scene texts。
    let labels: Vec<&str> = rendered
        .primitives
        .texts
        .iter()
        .map(|t| t.content.as_ref())
        .collect();
    assert!(
        labels.iter().any(|t| *t == "Confirm"),
        "title 'Confirm' should be rendered, got {labels:?}"
    );
    assert!(
        labels.iter().any(|t| *t == "Are you sure?"),
        "content 'Are you sure?' should be rendered, got {labels:?}"
    );
    assert!(
        labels.iter().any(|t| *t == "Cancel"),
        "action 'Cancel' should be rendered, got {labels:?}"
    );
    assert!(
        labels.iter().any(|t| *t == "OK"),
        "primary action 'OK' should be rendered, got {labels:?}"
    );
}

#[test]
fn modal_open_registers_focus_trap_on_card() {
    // 直接断言 card element 的 focus.scope.options.is_trap == true。
    let modal_element: Element<()> = Modal::new(true)
        .title("X")
        .action(ModalAction::primary("OK"))
        .into();

    // outer.children[1] (centered Stack) -> children[0] (card Flex).
    fn find_card_focus_trap<VM>(element: &Element<VM>, target: WidgetId) -> bool {
        if element.id == target {
            return element
                .focus
                .scope
                .as_ref()
                .map(|s| s.is_trap())
                .unwrap_or(false);
        }
        if let crate::ui::widget::common::WidgetKind::Container { children, .. } = &element.kind {
            for src in children {
                if let crate::ui::widget::common::ChildSource::Static(items) = src {
                    for child in items {
                        if find_card_focus_trap(child, target) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    let card_id = Modal::<()>::_card_id_of(&modal_element).expect("card id present");
    assert!(
        find_card_focus_trap(&modal_element, card_id),
        "card widget must have FocusScopeOptions::trap(true)"
    );
}

#[test]
fn modal_with_on_open_change_keeps_descriptor_attached() {
    let element: Element<()> = Modal::new(true)
        .on_open_change(ValueCommand::new(|_: &mut (), _: bool| {}))
        .close_on_backdrop_click(false)
        .into();
    let descriptor = element.modal.as_ref().expect("descriptor exists");
    assert!(descriptor.on_open_change.is_some());
    assert!(!descriptor.close_on_backdrop_click);
}
