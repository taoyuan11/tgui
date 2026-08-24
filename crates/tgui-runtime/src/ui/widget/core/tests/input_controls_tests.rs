use super::*;
use crate::foundation::binding::TextController;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::{ComponentThemes, Density, WidgetState};
use crate::ui::layout::Value;
use crate::ui::widget::{
    Calendar, CalendarChangeTrigger, CalendarSelectionChange, ColorPicker, DatePicker,
    FileDropEvent, NumberInput, NumberInputChange, NumberInputChangeTrigger, PopoverStyle,
    ResolvedElement, TimePicker, Upload, UploadFile, UploadFileId, UploadSelection, UploadStatus,
};
use chrono::{Datelike, NaiveDate, NaiveTime};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct TemporaryUploadFile {
    path: PathBuf,
}

impl TemporaryUploadFile {
    fn new(name: &str, contents: &[u8]) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);

        let directory = std::env::temp_dir().join(format!(
            "tgui-upload-test-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).expect("create upload test directory");
        let path = directory.join(name);
        std::fs::write(&path, contents).expect("write upload test file");
        Self { path }
    }
}

impl Drop for TemporaryUploadFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        if let Some(directory) = self.path.parent() {
            let _ = std::fs::remove_dir(directory);
        }
    }
}

fn resolved_children<VM>(element: &ResolvedElement<VM>) -> &[ResolvedElement<VM>] {
    match &element.kind {
        ResolvedWidgetKind::Container { children, .. }
        | ResolvedWidgetKind::Virtual { children, .. } => children,
        _ => &[],
    }
}

fn collect_button_commands<VM>(
    element: &ResolvedElement<VM>,
    expected_label: &str,
    commands: &mut Vec<Command<VM>>,
) {
    if matches!(
        &element.kind,
        ResolvedWidgetKind::Button { label, .. } if label.resolve() == expected_label
    ) {
        if let Some(command) = element.interactions.on_click.as_ref() {
            commands.push(command.clone());
        }
    }
    for child in resolved_children(element) {
        collect_button_commands(child, expected_label, commands);
    }
}

fn button_commands<VM>(element: &ResolvedElement<VM>, expected_label: &str) -> Vec<Command<VM>> {
    let mut commands = Vec::new();
    collect_button_commands(element, expected_label, &mut commands);
    commands
}

fn first_slider_change<VM>(element: &ResolvedElement<VM>) -> Option<ValueCommand<VM, f32>> {
    if let ResolvedWidgetKind::Slider {
        on_change: Some(command),
        ..
    } = &element.kind
    {
        return Some(command.clone());
    }
    resolved_children(element)
        .iter()
        .find_map(first_slider_change)
}

fn first_file_drop<VM>(element: &ResolvedElement<VM>) -> Option<ValueCommand<VM, FileDropEvent>> {
    if let Some(command) = element.interactions.on_file_drop.as_ref() {
        return Some(command.clone());
    }
    resolved_children(element).iter().find_map(first_file_drop)
}

#[test]
fn calendar_renders_month_grid_and_today_action() {
    let mut theme = Theme::default();
    theme.density = Density::Comfortable;
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
    assert!(!labels.contains(&"chevron_left"));
    assert!(!labels.contains(&"chevron_right"));
    assert!(
        rendered.primitives.textures.len() >= 2,
        "calendar navigation icons should render as SVG textures"
    );
    assert!(labels.iter().filter(|label| **label == "6").count() >= 1);
    assert!(computed.hit_regions.iter().all(|region| {
        !matches!(region.interaction, HitInteraction::Widget { .. })
            || (region.rect.x >= dp(12.0) && region.rect.right() <= dp(308.0))
    }));
}

#[test]
fn calendar_renders_minimum_and_maximum_supported_months_without_panicking() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();

    for month in [NaiveDate::MIN, NaiveDate::MAX] {
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(Calendar::new(month, Some(month)).today(None));

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

        assert!(computed
            .rendered()
            .primitives
            .texts
            .iter()
            .any(|text| text.content.as_ref() == month.day().to_string()));
    }
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
                    .open(true)
                    .on_open_change(ValueCommand::new(|_: &mut (), _: bool| {})),
            ),
    );

    let computed = tree.compute_scene(
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
    let rendered = computed.rendered();
    let overlay_labels = rendered
        .primitives
        .overlay_texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect::<Vec<_>>();

    assert!(overlay_labels.contains(&"June 2026"));
    assert!(overlay_labels.contains(&"Hour"));
    assert!(overlay_labels.contains(&"Minute"));
    assert!(overlay_labels.contains(&"09"));
    assert!(overlay_labels.contains(&"30"));
    assert!(overlay_labels.contains(&"Done"));
    assert!(!overlay_labels.contains(&"09:00"));
    assert!(!overlay_labels.contains(&"10:00"));
    assert!(!overlay_labels.contains(&"schedule"));
    let overlay_widget_hits = computed
        .overlay_hit_regions
        .iter()
        .filter(|region| matches!(region.interaction, HitInteraction::Widget { .. }))
        .count();
    assert!(
        overlay_widget_hits >= 12,
        "date and time picker overlays should expose clickable day, wheel, and done controls"
    );
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
    assert!(!trigger_labels.contains(&"calendar_today"));
    assert!(!trigger_labels.contains(&"schedule"));
    assert!(
        rendered.primitives.textures.len() + rendered.primitives.overlay_textures.len() >= 4,
        "picker trigger and navigation icons should render as SVG textures"
    );
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
fn picker_triggers_follow_runtime_density_and_style_on_the_same_tree() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 480.0, 160.0);
    let date: WidgetTree<()> = WidgetTree::new(
        DatePicker::new(
            "2026-06-06",
            Some(NaiveDate::from_ymd_opt(2026, 6, 6).unwrap()),
            NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
        )
        .style(|style, context| {
            style.width = match context.density {
                Density::Compact => dp(240.0),
                Density::Comfortable => dp(280.0),
                Density::Spacious => dp(320.0),
            };
        }),
    );
    let time: WidgetTree<()> = WidgetTree::new(
        TimePicker::new("09:30", NaiveTime::from_hms_opt(9, 30, 0)).style(|style, context| {
            style.width = match context.density {
                Density::Compact => dp(250.0),
                Density::Comfortable => dp(290.0),
                Density::Spacious => dp(330.0),
            };
        }),
    );
    let color: WidgetTree<()> = WidgetTree::new(ColorPicker::new(Color::hexa(0x3366CCFF)).style(
        |style, context| {
            style.width = match context.density {
                Density::Compact => dp(260.0),
                Density::Comfortable => dp(300.0),
                Density::Spacious => dp(340.0),
            };
        },
    ));

    for (density, height, gap, date_width, time_width, color_width) in [
        (
            Density::Compact,
            dp(32.0),
            dp(4.0),
            dp(240.0),
            dp(250.0),
            dp(260.0),
        ),
        (
            Density::Comfortable,
            dp(40.0),
            dp(6.0),
            dp(280.0),
            dp(290.0),
            dp(300.0),
        ),
        (
            Density::Spacious,
            dp(48.0),
            dp(8.0),
            dp(320.0),
            dp(330.0),
            dp(340.0),
        ),
    ] {
        let mut theme = Theme::light();
        theme.density = density;
        for (tree, expected_width, expected_gap) in [
            (&date, date_width, Some(gap)),
            (&time, time_width, Some(gap)),
            (&color, color_width, None),
        ] {
            let mut animations = AnimationEngine::default();
            let layout = tree.build_scene_layout(
                &font_manager,
                &theme,
                &media,
                &mut animations,
                UnitContext::default(),
                &HashMap::new(),
                &HashMap::new(),
                viewport,
            );
            assert_eq!(
                layout.resolved_root.layout.width,
                Some(Value::Static(crate::ui::layout::Length::Px(expected_width)))
            );
            assert_eq!(
                layout.resolved_root.layout.height,
                Some(Value::Static(crate::ui::layout::Length::Px(height)))
            );
            let ResolvedWidgetKind::Container {
                layout: container_layout,
                children,
                ..
            } = &layout.resolved_root.kind
            else {
                panic!("picker trigger should remain a flat container");
            };
            assert_eq!(children.len(), 2);
            if let Some(expected_gap) = expected_gap {
                assert_eq!(
                    container_layout.gap,
                    Value::Static(crate::ui::layout::Length::Px(expected_gap))
                );
            } else {
                let ResolvedWidgetKind::Container {
                    layout: overlay_layout,
                    ..
                } = &children[1].kind
                else {
                    panic!("color picker visual overlay should remain a container");
                };
                assert_eq!(
                    overlay_layout.gap,
                    Value::Static(crate::ui::layout::Length::Px(gap))
                );
            }
        }
    }
}

#[test]
fn calendar_and_picker_popovers_follow_runtime_density_on_the_same_tree() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 960.0, 720.0);
    let calendar: WidgetTree<()> = WidgetTree::new(
        Calendar::new(
            NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
            Some(NaiveDate::from_ymd_opt(2026, 6, 6).unwrap()),
        )
        .style(|style, context| match context.density {
            Density::Compact => {
                style.panel_width = dp(280.0);
                style.day_size = dp(24.0);
                style.gap = dp(2.0);
            }
            Density::Comfortable => {}
            Density::Spacious => {
                style.panel_width = dp(380.0);
                style.day_size = dp(40.0);
                style.gap = dp(10.0);
            }
        }),
    );
    let date: WidgetTree<()> = WidgetTree::new(
        DatePicker::new(
            "2026-06-06",
            Some(NaiveDate::from_ymd_opt(2026, 6, 6).unwrap()),
            NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
        )
        .open(true)
        .style(|style, context| {
            style.calendar.day_size = match context.density {
                Density::Compact => dp(24.0),
                Density::Comfortable => dp(32.0),
                Density::Spacious => dp(40.0),
            };
        }),
    );
    let time: WidgetTree<()> = WidgetTree::new(
        TimePicker::new("09:30", NaiveTime::from_hms_opt(9, 30, 0))
            .open(true)
            .style(|style, context| {
                let spacious = context.density == Density::Spacious;
                style.width = if spacious { dp(380.0) } else { dp(280.0) };
                style.option_width = if spacious { dp(140.0) } else { dp(100.0) };
            }),
    );
    let color: WidgetTree<()> =
        WidgetTree::new(ColorPicker::new(Color::hexa(0x3366CCFF)).open(true).style(
            |style, context| {
                let spacious = context.density == Density::Spacious;
                style.width = if spacious { dp(380.0) } else { dp(280.0) };
                style.swatch_size = if spacious { dp(36.0) } else { dp(20.0) };
            },
        ));

    for (density, panel_width, day_size, calendar_gap, option_width, swatch_size) in [
        (
            Density::Compact,
            dp(280.0),
            dp(24.0),
            dp(2.0),
            dp(100.0),
            dp(20.0),
        ),
        (
            Density::Spacious,
            dp(380.0),
            dp(40.0),
            dp(10.0),
            dp(140.0),
            dp(36.0),
        ),
    ] {
        let mut theme = Theme::light();
        theme.density = density;
        let mut animations = AnimationEngine::default();
        let layout = calendar.build_scene_layout(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            UnitContext::default(),
            &HashMap::new(),
            &HashMap::new(),
            viewport,
        );
        assert_eq!(
            layout.resolved_root.layout.width,
            Some(Value::Static(crate::ui::layout::Length::Px(panel_width)))
        );
        let ResolvedWidgetKind::Container { children, .. } = &layout.resolved_root.kind else {
            panic!("calendar root should remain a flat container");
        };
        let ResolvedWidgetKind::Container {
            layout: days_layout,
            children: days,
            ..
        } = &children[2].kind
        else {
            panic!("calendar days should remain a grid container");
        };
        assert_eq!(
            days_layout.gap,
            Value::Static(crate::ui::layout::Length::Px(calendar_gap))
        );
        assert_eq!(
            days[0].layout.width,
            Some(Value::Static(crate::ui::layout::Length::Px(day_size)))
        );
        assert_eq!(
            days[0].layout.height,
            Some(Value::Static(crate::ui::layout::Length::Px(day_size)))
        );

        let mut animations = AnimationEngine::default();
        let date_scene = date.compute_scene(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            viewport,
            None,
            None,
            None,
            None,
            false,
        );
        let date_day_hits = date_scene
            .overlay_hit_regions
            .iter()
            .filter(|region| {
                matches!(region.interaction, HitInteraction::Widget { .. })
                    && (region.rect.width - day_size).abs() <= dp(0.1)
                    && (region.rect.height - day_size).abs() <= dp(0.1)
            })
            .count();
        assert!(
            date_day_hits >= 42,
            "density={density:?}, hits={date_day_hits}"
        );

        let mut animations = AnimationEngine::default();
        let time_scene = time.compute_scene(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            viewport,
            None,
            None,
            None,
            None,
            false,
        );
        let time_option_hits = time_scene
            .overlay_hit_regions
            .iter()
            .filter(|region| {
                matches!(region.interaction, HitInteraction::Widget { .. })
                    && (region.rect.width - option_width).abs() <= dp(0.1)
            })
            .count();
        assert!(
            time_option_hits >= 6,
            "density={density:?}, hits={time_option_hits}"
        );

        let mut animations = AnimationEngine::default();
        let color_scene = color.compute_scene(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            viewport,
            None,
            None,
            None,
            None,
            false,
        );
        let swatch_hits = color_scene
            .overlay_hit_regions
            .iter()
            .filter(|region| {
                matches!(region.interaction, HitInteraction::Widget { .. })
                    && (region.rect.width - swatch_size).abs() <= dp(0.1)
                    && (region.rect.height - swatch_size).abs() <= dp(0.1)
            })
            .count();
        assert!(swatch_hits >= 8, "density={density:?}, hits={swatch_hits}");
    }
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
        Rect::new(0.0, 0.0, 340.0, 64.0),
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
fn advanced_inputs_apply_base_component_theme_overrides_in_the_real_scene() {
    let input_color = Color::hexa(0x123456FF);
    let button_color = Color::hexa(0xA12B3CFF);
    let select_menu_color = Color::hexa(0x1B4332FF);
    let mut theme = Theme::light();
    theme.components = ComponentThemes::default()
        .input(move |style, _| {
            style.background = StateValue::new(Value::Static(input_color));
        })
        .button(move |style, _| {
            style.background = StateValue::new(Value::Static(button_color));
        })
        .select(move |style, _| {
            style.menu_background = Value::Static(select_menu_color);
        });

    let cases: Vec<(&str, WidgetTree<()>, bool)> = vec![
        (
            "number",
            WidgetTree::new(NumberInput::new("12", Some(12.0))),
            false,
        ),
        (
            "date",
            WidgetTree::new(
                DatePicker::new(
                    "2026-06-06",
                    Some(NaiveDate::from_ymd_opt(2026, 6, 6).unwrap()),
                    NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
                )
                .open(true),
            ),
            true,
        ),
        (
            "time",
            WidgetTree::new(TimePicker::new("09:30", NaiveTime::from_hms_opt(9, 30, 0)).open(true)),
            true,
        ),
        (
            "color",
            WidgetTree::new(ColorPicker::new(Color::hexa(0x3366CCFF)).open(true)),
            true,
        ),
        (
            "upload",
            WidgetTree::new(
                Upload::new(Vec::new()).on_select(ValueCommand::new(|_: &mut (), _| {})),
            ),
            false,
        ),
    ];
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();

    for (name, tree, has_picker_panel) in cases {
        let mut animations = AnimationEngine::default();
        let computed = tree.compute_scene(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 640.0, 520.0),
            None,
            None,
            None,
            None,
            false,
        );
        let rendered = computed.rendered();
        let has_color = |color| {
            rendered
                .primitives
                .shapes
                .iter()
                .chain(rendered.primitives.overlay_shapes.iter())
                .any(|shape| shape.color == color)
        };
        assert!(
            has_color(input_color),
            "{name} must use the Input component theme"
        );
        assert!(
            has_color(button_color),
            "{name} must use the Button component theme"
        );
        if has_picker_panel {
            assert!(
                has_color(select_menu_color),
                "{name} picker panel must use the Select menu component token"
            );
        }
    }
}

#[test]
fn number_input_stepper_geometry_follows_runtime_theme_density() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let tree: WidgetTree<()> =
        WidgetTree::new(NumberInput::new("12", Some(12.0)).style(|style, context| {
            style.width = match context.density {
                Density::Compact => dp(160.0),
                Density::Comfortable => dp(180.0),
                Density::Spacious => dp(220.0),
            };
        }));
    for (density, field_width, height, button_width, gap) in [
        (Density::Compact, dp(160.0), dp(32.0), dp(32.0), dp(4.0)),
        (Density::Spacious, dp(220.0), dp(48.0), dp(48.0), dp(8.0)),
    ] {
        let mut theme = Theme::light();
        theme.density = density;
        let mut animations = AnimationEngine::default();
        let computed = tree.compute_scene(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 380.0, 72.0),
            None,
            None,
            None,
            None,
            false,
        );

        let stepper_rects = computed
            .hit_regions
            .iter()
            .filter_map(|region| match &region.interaction {
                HitInteraction::Widget { .. } if region.rect.width <= dp(52.0) => Some(region.rect),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            stepper_rects.len(),
            2,
            "density={density:?}, stepper_rects={stepper_rects:?}"
        );
        assert!(stepper_rects.iter().all(|rect| {
            (rect.width - button_width).abs() <= dp(0.1) && (rect.height - height).abs() <= dp(0.1)
        }));

        let input_rect = computed
            .hit_regions
            .iter()
            .find_map(|region| match region.interaction {
                HitInteraction::TextInput { .. } => Some(region.rect),
                _ => None,
            })
            .expect("number input should expose its text field");
        assert!((input_rect.width - field_width).abs() <= dp(0.1));
        // The public Input owns its internal text hit geometry; NumberInput only
        // controls the surrounding stepper geometry and spacing.
        assert!(input_rect.height > dp(0.0));

        let total_width = field_width + button_width * 2.0 + gap * 2.0;
        let left = stepper_rects[0].x.min(stepper_rects[1].x);
        let right = stepper_rects[0].right().max(stepper_rects[1].right());
        assert!((right - left - total_width).abs() <= dp(0.1));
    }
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
    assert!(labels.contains(&"Current color"));
    assert!(!labels.contains(&"palette"));
    assert!(!labels.contains(&"keyboard_arrow_down"));
    assert!(
        rendered.primitives.textures.len() + rendered.primitives.overlay_textures.len() >= 2,
        "color picker icons should render as SVG textures"
    );
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
    let mut theme = Theme::default();
    theme.density = Density::Comfortable;
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
    assert!(!labels.contains(&"upload_file"));
    assert!(!labels.contains(&"description"));
    assert!(!labels.contains(&"pending"));
    assert!(!labels.contains(&"delete"));
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
        .textures
        .iter()
        .find(|texture| {
            let badge_center_x = upload_badge.rect.x + upload_badge.rect.width * 0.5;
            let badge_center_y = upload_badge.rect.y + upload_badge.rect.height * 0.5;
            let icon_center_x = texture.frame.x + texture.frame.width * 0.5;
            let icon_center_y = texture.frame.y + texture.frame.height * 0.5;
            (icon_center_x - badge_center_x).abs() <= dp(0.5)
                && (icon_center_y - badge_center_y).abs() <= dp(0.5)
        })
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
        rendered.primitives.textures.len() >= 7,
        "upload badge, file, status, and remove icons should render as SVG textures"
    );
}

#[test]
fn upload_density_and_drop_state_are_resolved_in_the_real_scene() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let file = UploadFile {
        id: UploadFileId::new("density"),
        path: PathBuf::from("density.png"),
        name: "density.png".to_string(),
        size_bytes: Some(2048),
        status: UploadStatus::Uploading { progress: 0.5 },
    };
    let tree: WidgetTree<()> = WidgetTree::new(
        Upload::new(vec![file])
            .on_select(ValueCommand::new(|_: &mut (), _| {}))
            .on_remove(ValueCommand::new(|_: &mut (), _| {})),
    );
    for (density, width, min_height, badge_size, action_size) in [
        (Density::Compact, dp(420.0), dp(112.0), dp(36.0), dp(28.0)),
        (Density::Spacious, dp(500.0), dp(160.0), dp(52.0), dp(40.0)),
    ] {
        let mut theme = Theme::light();
        theme.density = density;
        let mut animations = AnimationEngine::default();
        let normal = tree.compute_scene(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 560.0, 440.0),
            None,
            None,
            None,
            None,
            false,
        );
        let (drop_id, drop_rect) = normal
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::Widget {
                    id, interactions, ..
                } if interactions.on_file_drop.is_some() => Some((*id, region.rect)),
                _ => None,
            })
            .expect("upload drop zone should be interactive");
        assert!((drop_rect.width - width).abs() <= dp(0.1));
        assert!(drop_rect.height + dp(0.1) >= min_height);

        let rendered = normal.rendered();
        assert!(rendered.primitives.shapes.iter().any(|shape| {
            (shape.rect.width - badge_size).abs() <= dp(0.1)
                && (shape.rect.height - badge_size).abs() <= dp(0.1)
        }));
        assert!(normal.hit_regions.iter().any(|region| {
            matches!(region.interaction, HitInteraction::Widget { .. })
                && (region.rect.width - action_size).abs() <= dp(0.1)
                && (region.rect.height - action_size).abs() <= dp(0.1)
        }));

        let normal_background = rendered
            .primitives
            .shapes
            .iter()
            .find(|shape| {
                shape.stroke_width == dp(0.0)
                    && (shape.rect.x - (drop_rect.x + theme.border.thin)).abs() <= dp(0.1)
                    && (shape.rect.y - (drop_rect.y + theme.border.thin)).abs() <= dp(0.1)
                    && (shape.rect.width - (drop_rect.width - theme.border.thin * 2.0)).abs()
                        <= dp(0.1)
                    && (shape.rect.height - (drop_rect.height - theme.border.thin * 2.0)).abs()
                        <= dp(0.1)
            })
            .unwrap_or_else(|| {
                panic!(
                    "drop zone should render a background: density={density:?}, drop={drop_rect:?}, shapes={:?}",
                    rendered
                        .primitives
                        .shapes
                        .iter()
                        .map(|shape| (shape.rect, shape.stroke_width, shape.color))
                        .collect::<Vec<_>>()
                )
            })
            .color;
        assert_eq!(normal_background, theme.colors.surface);

        let mut states = WidgetStateMap::default();
        states.set(
            drop_id,
            WidgetState {
                hovered: true,
                ..Default::default()
            },
        );
        let hovered = tree.compute_scene_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            false,
            None,
            None,
            &states,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 560.0, 440.0),
            None,
            None,
            None,
            None,
            false,
        );
        let hovered_background = hovered
            .rendered()
            .primitives
            .shapes
            .iter()
            .find(|shape| {
                shape.stroke_width == dp(0.0)
                    && (shape.rect.x - (drop_rect.x + theme.border.thin)).abs() <= dp(0.1)
                    && (shape.rect.y - (drop_rect.y + theme.border.thin)).abs() <= dp(0.1)
                    && (shape.rect.width - (drop_rect.width - theme.border.thin * 2.0)).abs()
                        <= dp(0.1)
                    && (shape.rect.height - (drop_rect.height - theme.border.thin * 2.0)).abs()
                        <= dp(0.1)
            })
            .expect("hovered drop zone should render a background")
            .color;
        assert_eq!(hovered_background, theme.colors.primary_container);
    }
}

#[test]
fn number_input_decimal_step_updates_text_and_emits_the_displayed_value() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let controller: TextController = "1.2".into();
    let tree: WidgetTree<Vec<NumberInputChange>> = WidgetTree::new(
        NumberInput::new(controller.clone(), Some(1.2))
            .range(10.0, -10.0)
            .step(0.001)
            .on_change(ValueCommand::new(
                |changes: &mut Vec<NumberInputChange>, change| changes.push(change),
            )),
    );
    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 360.0, 80.0),
    );
    let commands = button_commands(&layout.resolved_root, "Increase value");
    assert_eq!(commands.len(), 1);

    let mut changes = Vec::new();
    commands[0].execute(&mut changes);

    assert_eq!(controller.text(), "1.201");
    assert_eq!(
        changes,
        vec![NumberInputChange {
            value: Some(1.201),
            text: "1.201".to_string(),
            trigger: NumberInputChangeTrigger::StepUp,
        }]
    );
}

#[test]
fn calendar_static_month_navigation_updates_the_existing_tree() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<Vec<CalendarSelectionChange>> = WidgetTree::new(
        Calendar::new(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(), None)
            .today(None)
            .on_change(ValueCommand::new(
                |changes: &mut Vec<CalendarSelectionChange>, change| changes.push(change),
            )),
    );
    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 360.0, 360.0),
    );
    let previous = button_commands(&layout.resolved_root, "Previous month");
    let next = button_commands(&layout.resolved_root, "Next month");
    assert_eq!(previous.len(), 1);
    assert_eq!(next.len(), 1);

    let mut changes = Vec::new();
    next[0].execute(&mut changes);
    let rendered = tree.render_output(
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

    assert!(rendered
        .primitives
        .texts
        .iter()
        .any(|text| text.content.as_ref() == "July 2026"));
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].trigger, CalendarChangeTrigger::NextMonth);
    assert_eq!(
        changes[0].display_month,
        NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()
    );
}

#[test]
fn static_date_and_time_pickers_update_without_callbacks_and_close() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 420.0, 420.0);

    let date_controller: TextController = "2026-06-06".into();
    let date_tree: WidgetTree<()> = WidgetTree::new(
        DatePicker::new(
            date_controller.clone(),
            Some(NaiveDate::from_ymd_opt(2026, 6, 6).unwrap()),
            NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
        )
        .open(true),
    );
    let mut animations = AnimationEngine::default();
    let date_layout = date_tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let date_content = date_layout
        .resolved_root
        .popover
        .as_ref()
        .expect("date picker should own a popover")
        .content
        .as_ref()
        .clone();
    let date_content_tree = WidgetTree::new(date_content);
    let date_content_layout = date_content_tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let date_commands = button_commands(&date_content_layout.resolved_root, "15");
    assert_eq!(date_commands.len(), 1);
    date_commands[0].execute(&mut ());
    assert_eq!(date_controller.text(), "2026-06-15");
    let closed_date = date_tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    assert!(closed_date.primitives.overlay_texts.is_empty());

    let time_controller: TextController = "09:07".into();
    let time_tree: WidgetTree<()> = WidgetTree::new(
        TimePicker::new(
            time_controller.clone(),
            Some(NaiveTime::from_hms_opt(9, 7, 0).unwrap()),
        )
        .minute_step(15)
        .open(true),
    );
    let time_layout = time_tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let time_content = time_layout
        .resolved_root
        .popover
        .as_ref()
        .expect("time picker should own a popover")
        .content
        .as_ref()
        .clone();
    let time_content_tree = WidgetTree::new(time_content);
    let time_content_layout = time_content_tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let hour_commands = button_commands(&time_content_layout.resolved_root, "10");
    assert_eq!(hour_commands.len(), 1);
    hour_commands[0].execute(&mut ());
    assert_eq!(time_controller.text(), "10:07");
    let done = button_commands(&time_content_layout.resolved_root, "Done");
    assert_eq!(done.len(), 1);
    done[0].execute(&mut ());
    let closed_time = time_tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    assert!(closed_time.primitives.overlay_texts.is_empty());
}

#[test]
fn static_color_picker_slider_updates_without_an_external_callback() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 420.0, 420.0);
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> =
        WidgetTree::new(ColorPicker::new(Color::hexa(0x3366CCFF)).open(true));
    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let content = layout
        .resolved_root
        .popover
        .as_ref()
        .expect("color picker should own a popover")
        .content
        .as_ref()
        .clone();
    let content_tree = WidgetTree::new(content);
    let content_layout = content_tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    first_slider_change(&content_layout.resolved_root)
        .expect("red channel slider should expose a change command")
        .execute(&mut (), 128.0);

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    assert!(rendered
        .primitives
        .texts
        .iter()
        .chain(rendered.primitives.overlay_texts.iter())
        .any(|text| text.content.as_ref() == "#8066CCFF"));
}

#[test]
fn upload_static_and_signal_lists_update_and_directories_are_rejected() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 520.0, 420.0);
    let file = UploadFile::from_path(PathBuf::from("first.txt"));

    let mut animations = AnimationEngine::default();
    let static_tree: WidgetTree<()> = WidgetTree::new(Upload::new(vec![file.clone()]));
    let static_layout = static_tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let remove = button_commands(&static_layout.resolved_root, "Remove file");
    assert_eq!(remove.len(), 1);
    remove[0].execute(&mut ());
    let removed = static_tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    assert!(!removed
        .primitives
        .texts
        .iter()
        .any(|text| text.content.as_ref() == "first.txt"));

    let context = test_context();
    let files = context.state(vec![file]);
    let signal_tree: WidgetTree<()> = WidgetTree::new(Upload::new(files.signal()));
    files.set(vec![UploadFile::from_path(PathBuf::from("second.txt"))]);
    let updated = signal_tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    assert!(updated
        .primitives
        .texts
        .iter()
        .any(|text| text.content.as_ref() == "second.txt"));

    let drop_tree: WidgetTree<Vec<UploadSelection>> =
        WidgetTree::new(Upload::new(Vec::new()).on_select(ValueCommand::new(
            |selections: &mut Vec<UploadSelection>, selection| selections.push(selection),
        )));
    let drop_layout = drop_tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let drop = first_file_drop(&drop_layout.resolved_root)
        .expect("upload drop zone should expose its file-drop command");
    let mut selections = Vec::new();
    drop.execute(
        &mut selections,
        FileDropEvent {
            position: Point::new(10.0, 10.0),
            paths: vec![PathBuf::from(".")],
        },
    );
    assert_eq!(selections.len(), 1);
    assert!(selections[0].files.is_empty());
    assert_eq!(selections[0].rejected.len(), 1);
    assert_eq!(
        selections[0].rejected[0].reason,
        "Only files can be uploaded"
    );
}

#[test]
fn upload_drop_preserves_the_dispatching_command_context() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let upload_file = TemporaryUploadFile::new("context.txt", b"context");
    let tree: WidgetTree<Vec<UploadSelection>> = WidgetTree::new(
        Upload::new(Vec::new()).on_select(ValueCommand::new_with_context(
            |selections: &mut Vec<UploadSelection>, selection, context| {
                selections.push(selection);
                context.request_rebuild();
            },
        )),
    );
    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 520.0, 420.0),
    );
    let drop = first_file_drop(&layout.resolved_root)
        .expect("upload drop zone should expose its file-drop command");
    let context = CommandContext::detached();
    let revision_before = context.root_rebuild_revision();
    let mut selections = Vec::new();

    drop.execute_with_context(
        &mut selections,
        FileDropEvent {
            position: Point::new(10.0, 10.0),
            paths: vec![upload_file.path.clone()],
        },
        &context,
    );

    assert_eq!(selections.len(), 1);
    assert_eq!(selections[0].files.len(), 1);
    assert_eq!(context.root_rebuild_revision(), revision_before + 1);
}
