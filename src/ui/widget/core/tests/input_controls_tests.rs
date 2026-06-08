use super::*;
use crate::ui::widget::{
    Calendar, ColorPicker, DatePicker, NumberInput, PopoverStyle, TimePicker, Upload, UploadFile,
    UploadFileId, UploadStatus,
};
use chrono::{NaiveDate, NaiveTime};
use std::path::PathBuf;

#[test]
fn calendar_renders_month_grid_and_today_action() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(Calendar::new(
        NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
        Some(NaiveDate::from_ymd_opt(2026, 6, 6).unwrap()),
    ));

    let computed = tree.compute_scene(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 360.0, 360.0),
        None,
        None,
        None,
        None,
        false,
    );
    let rendered = computed.rendered();
    let labels = rendered
        .primitives
        .texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"June 2026"));
    assert!(labels.contains(&"Today"));
    assert!(labels.contains(&"chevron_left"));
    assert!(labels.contains(&"chevron_right"));
    assert!(labels.iter().filter(|label| **label == "6").count() >= 1);
    assert!(computed.hit_regions.iter().all(|region| {
        !matches!(region.interaction, HitInteraction::Widget { .. })
            || (region.rect.x >= dp(12.0) && region.rect.right() <= dp(308.0))
    }));
}

#[test]
fn date_and_time_pickers_emit_popover_content_when_open() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Flex::vertical()
            .gap(dp(12.0))
            .child(
                DatePicker::new(
                    "2026-06-06",
                    Some(NaiveDate::from_ymd_opt(2026, 6, 6).unwrap()),
                    NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
                )
                .open(true),
            )
            .child(
                TimePicker::new("09:30", NaiveTime::from_hms_opt(9, 30, 0))
                    .minute_step(60)
                    .open(true),
            ),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 420.0, 420.0),
        None,
        None,
        None,
        None,
        false,
    );
    let overlay_labels = rendered
        .primitives
        .overlay_texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect::<Vec<_>>();

    assert!(overlay_labels.contains(&"June 2026"));
    assert!(overlay_labels.contains(&"schedule"));
    assert!(overlay_labels.contains(&"09:00"));
    assert!(overlay_labels.contains(&"10:00"));
    let panel_background_count = rendered
        .primitives
        .overlay_shapes
        .iter()
        .filter(|shape| {
            shape.color
                == PopoverStyle::default_for_theme(&Theme::dark())
                    .background
                    .resolve()
        })
        .count();
    assert!(panel_background_count >= 4);

    let trigger_labels = rendered
        .primitives
        .texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect::<Vec<_>>();
    assert!(trigger_labels.contains(&"calendar_today"));
    assert!(trigger_labels.contains(&"schedule"));
}

#[test]
fn picker_popovers_render_panel_background_in_light_theme() {
    let theme = Theme::light();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Flex::vertical()
            .gap(dp(12.0))
            .child(
                DatePicker::new(
                    "2026-06-06",
                    Some(NaiveDate::from_ymd_opt(2026, 6, 6).unwrap()),
                    NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
                )
                .open(true),
            )
            .child(ColorPicker::new(Color::hexa(0x3366CCFF)).open(true)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 420.0, 420.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered.primitives.overlay_shapes.iter().any(|shape| {
        shape.color
            == PopoverStyle::default_for_theme(&Theme::light())
                .background
                .resolve()
    }));
}

#[test]
fn number_input_combines_text_field_and_step_buttons() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        NumberInput::new("12", Some(12.0))
            .range(0.0, 99.0)
            .step(1.0),
    );

    let computed = tree.compute_scene(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 260.0, 64.0),
        None,
        None,
        None,
        None,
        false,
    );
    let button_hits = computed
        .hit_regions
        .iter()
        .filter(|region| matches!(region.interaction, HitInteraction::Widget { .. }))
        .count();
    let text_inputs = computed
        .hit_regions
        .iter()
        .filter(|region| matches!(region.interaction, HitInteraction::TextInput { .. }))
        .count();

    assert!(button_hits >= 2);
    assert_eq!(text_inputs, 1);
}

#[test]
fn color_picker_open_renders_channel_sliders_and_swatches() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> =
        WidgetTree::new(ColorPicker::new(Color::hexa(0x3366CCFF)).open(true));

    let computed = tree.compute_scene(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 420.0, 360.0),
        None,
        None,
        None,
        None,
        false,
    );
    let slider_hits = computed
        .overlay_hit_regions
        .iter()
        .filter(|region| matches!(region.interaction, HitInteraction::Slider { .. }))
        .count();
    let swatch_hits = computed
        .overlay_hit_regions
        .iter()
        .filter(|region| matches!(region.interaction, HitInteraction::Widget { .. }))
        .count();

    assert_eq!(slider_hits, 4);
    assert!(swatch_hits >= 8);

    let rendered = computed.rendered();
    let labels = rendered
        .primitives
        .texts
        .iter()
        .chain(rendered.primitives.overlay_texts.iter())
        .map(|text| text.content.as_ref())
        .collect::<Vec<_>>();
    assert!(labels.contains(&"palette"));
    assert!(labels.contains(&"keyboard_arrow_down"));
    assert!(labels.contains(&"Current color"));
    let panel_background_count = rendered
        .primitives
        .overlay_shapes
        .iter()
        .filter(|shape| {
            shape.color
                == PopoverStyle::default_for_theme(&Theme::dark())
                    .background
                    .resolve()
        })
        .count();
    assert!(panel_background_count >= 2);
}

#[test]
fn upload_renders_drop_handler_queue_progress_and_remove_action() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let files = vec![
        UploadFile {
            id: UploadFileId::new("queued"),
            path: PathBuf::from("queued.pdf"),
            name: "queued.pdf".to_string(),
            size_bytes: Some(1200),
            status: UploadStatus::Queued,
        },
        UploadFile {
            id: UploadFileId::new("uploading"),
            path: PathBuf::from("uploading.png"),
            name: "uploading.png".to_string(),
            size_bytes: Some(2048),
            status: UploadStatus::Uploading { progress: 0.5 },
        },
    ];
    let tree: WidgetTree<()> = WidgetTree::new(
        Upload::new(files)
            .on_select(ValueCommand::new(|_: &mut (), _| {}))
            .on_remove(ValueCommand::new(|_: &mut (), _| {})),
    );

    let computed = tree.compute_scene(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 520.0, 420.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(computed.hit_regions.iter().any(|region| {
        matches!(&region.interaction, HitInteraction::Widget { interactions, .. }
            if interactions.on_file_drop.is_some())
    }));

    let rendered = computed.rendered();
    let labels = rendered
        .primitives
        .texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect::<Vec<_>>();
    assert!(labels.contains(&"queued.pdf"));
    assert!(labels.contains(&"Uploading 50%"));
    assert!(labels.contains(&"upload_file"));
    let upload_badge = rendered
        .primitives
        .shapes
        .iter()
        .find(|shape| {
            (shape.rect.width - dp(44.0)).abs() <= dp(0.1)
                && (shape.rect.height - dp(44.0)).abs() <= dp(0.1)
        })
        .expect("upload badge should render");
    let upload_icon = rendered
        .primitives
        .texts
        .iter()
        .find(|text| text.content.as_ref() == "upload_file")
        .expect("upload icon should render");
    let badge_center_x = upload_badge.rect.x + upload_badge.rect.width * 0.5;
    let badge_center_y = upload_badge.rect.y + upload_badge.rect.height * 0.5;
    let icon_center_x = upload_icon.frame.x + upload_icon.frame.width * 0.5;
    let icon_center_y = upload_icon.frame.y + upload_icon.frame.height * 0.5;
    assert!(
        (icon_center_x - badge_center_x).abs() <= dp(0.1)
            && (icon_center_y - badge_center_y).abs() <= dp(0.1),
        "upload icon should be centered in its badge: badge={:?}, icon={:?}",
        upload_badge.rect,
        upload_icon.frame
    );
    assert!(
        labels
            .iter()
            .filter(|label| **label == "description")
            .count()
            >= 2
    );
    assert!(labels.iter().filter(|label| **label == "pending").count() >= 2);
    assert!(labels.iter().filter(|label| **label == "delete").count() >= 2);
}
