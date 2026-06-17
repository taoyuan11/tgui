use super::*;

impl<VM> ResolvedElement<VM> {
    pub(super) fn measure_context(&self) -> MeasureContext {
        match &self.kind {
            ResolvedWidgetKind::Container { .. } => MeasureContext::None,
            ResolvedWidgetKind::Virtual { .. } => MeasureContext::None,
            ResolvedWidgetKind::Text { text, .. } => MeasureContext::Text {
                id: self.id,
                text: text.clone(),
            },
            #[cfg(feature = "audio")]
            ResolvedWidgetKind::Audio { .. } => MeasureContext::Audio { id: self.id },
            ResolvedWidgetKind::Image { image, .. } => MeasureContext::Image {
                id: self.id,
                image: image.clone(),
            },
            ResolvedWidgetKind::Icon { .. } => MeasureContext::None,
            ResolvedWidgetKind::Canvas { scene, .. } => MeasureContext::Canvas {
                id: self.id,
                scene: scene.clone(),
            },
            #[cfg(feature = "video")]
            ResolvedWidgetKind::VideoSurface { video, .. } => MeasureContext::VideoSurface {
                id: self.id,
                video: video.clone(),
            },
            ResolvedWidgetKind::Button { label, style, .. } => MeasureContext::Button {
                id: self.id,
                label: label.clone(),
                style: style.clone(),
            },
            ResolvedWidgetKind::Checkbox { label, style, .. } => MeasureContext::Checkbox {
                id: self.id,
                label: label.clone(),
                style: style.clone(),
            },
            ResolvedWidgetKind::Radio { label, style, .. } => MeasureContext::Radio {
                id: self.id,
                label: label.clone(),
                style: style.clone(),
            },
            ResolvedWidgetKind::Switch { style, .. } => MeasureContext::Switch {
                id: self.id,
                style: style.clone(),
            },
            ResolvedWidgetKind::Select {
                selected_label,
                placeholder,
                style,
                ..
            } => MeasureContext::Select {
                id: self.id,
                selected_label: selected_label.clone(),
                placeholder: placeholder.clone(),
                style: style.clone(),
            },
            ResolvedWidgetKind::SelectOptionRow { .. } => MeasureContext::None,
            ResolvedWidgetKind::Slider {
                style, orientation, ..
            } => MeasureContext::Slider {
                id: self.id,
                style: style.clone(),
                orientation: *orientation,
            },
            ResolvedWidgetKind::ProgressBar {
                show_label,
                label,
                style,
                ..
            } => MeasureContext::ProgressBar {
                id: self.id,
                show_label: *show_label,
                label: label.clone(),
                style: style.clone(),
            },
            ResolvedWidgetKind::Spinner {
                style,
                size_override,
                ..
            } => MeasureContext::Spinner {
                id: self.id,
                style: style.clone(),
                size_override: size_override.clone(),
            },
            ResolvedWidgetKind::Divider {
                orientation,
                thickness_override,
                label,
                style,
                ..
            } => MeasureContext::Divider {
                id: self.id,
                orientation: *orientation,
                thickness_override: thickness_override.clone(),
                label: label.clone(),
                style: style.clone(),
            },
            ResolvedWidgetKind::TextEditor {
                controller,
                placeholder,
                style,
                multiline,
                ..
            } => MeasureContext::TextEditor {
                id: self.id,
                controller: controller.clone(),
                placeholder: placeholder.clone(),
                style: style.clone(),
                multiline: *multiline,
            },
            ResolvedWidgetKind::ToastHost { .. } => MeasureContext::None,
            ResolvedWidgetKind::Portal { .. } => MeasureContext::None,
        }
    }

    pub(super) fn build_layout_tree(
        &self,
        taffy: &mut TaffyTree<MeasureContext>,
        animations: &mut AnimationEngine,
        theme: &Theme,
        units: UnitContext,
        parent_kind: Option<ContainerKind>,
        viewport: Rect,
        is_root: bool,
        now: std::time::Instant,
    ) -> Result<LayoutNode, taffy::TaffyError> {
        super::tree::with_widget_stack_frame(|| {
            let owner = self.id.dependency_owner(DependencyPhase::Layout);
            track_dependency_scope(owner, || {
                self.build_layout_tree_tracked(
                    taffy,
                    animations,
                    theme,
                    units,
                    parent_kind,
                    viewport,
                    is_root,
                    now,
                )
            })
        })
    }

    fn build_layout_tree_tracked(
        &self,
        taffy: &mut TaffyTree<MeasureContext>,
        animations: &mut AnimationEngine,
        theme: &Theme,
        units: UnitContext,
        parent_kind: Option<ContainerKind>,
        viewport: Rect,
        is_root: bool,
        now: std::time::Instant,
    ) -> Result<LayoutNode, taffy::TaffyError> {
        let mut child_layouts = Vec::new();
        match &self.kind {
            ResolvedWidgetKind::Container {
                layout, children, ..
            } => {
                child_layouts.reserve(children.len());
                for child in children {
                    child_layouts.push(child.build_layout_tree(
                        taffy,
                        animations,
                        theme,
                        units,
                        Some(layout.kind.clone()),
                        viewport,
                        false,
                        now,
                    )?);
                }
            }
            ResolvedWidgetKind::Virtual { children, .. } => {
                child_layouts.reserve(children.len());
                for child in children {
                    child_layouts.push(child.build_layout_tree(
                        taffy, animations, theme, units, None, viewport, false, now,
                    )?);
                }
            }
            ResolvedWidgetKind::ToastHost { .. } => {}
            _ => {}
        }

        let style = self.taffy_style(
            parent_kind,
            viewport,
            is_root,
            animations,
            theme,
            units,
            now,
        );
        let node = if child_layouts.is_empty() {
            taffy.new_leaf_with_context(style, self.measure_context())?
        } else {
            let child_ids = child_layouts
                .iter()
                .map(|child| child.node)
                .collect::<Vec<_>>();
            taffy.new_with_children(style, &child_ids)?
        };

        Ok(LayoutNode {
            node,
            children: child_layouts,
        })
    }

    pub(super) fn taffy_style(
        &self,
        parent_kind: Option<ContainerKind>,
        viewport: Rect,
        is_root: bool,
        animations: &mut AnimationEngine,
        theme: &Theme,
        units: UnitContext,
        now: std::time::Instant,
    ) -> TaffyStyle {
        let default_min_width = match &self.kind {
            ResolvedWidgetKind::Select { .. } if self.layout.min_width.is_none() => {
                Dimension::from_length(0.0)
            }
            ResolvedWidgetKind::Slider {
                style, orientation, ..
            } if self.layout.min_width.is_none() => {
                let width = if orientation.is_horizontal() {
                    style.min_width
                } else {
                    style.min_height
                };
                Dimension::from_length(width.get())
            }
            ResolvedWidgetKind::ToastHost { .. } | ResolvedWidgetKind::Portal { .. } => {
                Dimension::from_length(0.0)
            }
            _ => Dimension::AUTO,
        };
        let width = self.layout.width.as_ref().map(|value| {
            track_property_scope(PropertySlot::Width, || {
                resolve_dimension(
                    value,
                    animations,
                    self.id,
                    WidgetProperty::Width,
                    now,
                    units,
                )
            })
        });
        let width = if is_root {
            width.or(Some(Dimension::from_length(viewport.width)))
        } else {
            width
        };
        let height = self.layout.height.as_ref().map(|value| {
            track_property_scope(PropertySlot::Height, || {
                resolve_dimension(
                    value,
                    animations,
                    self.id,
                    WidgetProperty::Height,
                    now,
                    units,
                )
            })
        });
        let height = if is_root {
            height.or(Some(Dimension::from_length(viewport.height)))
        } else {
            height
        };
        let default_min_height = match &self.kind {
            ResolvedWidgetKind::Slider {
                style, orientation, ..
            } if !orientation.is_horizontal() && self.layout.min_height.is_none() => {
                Dimension::from_length(style.min_width.get())
            }
            _ => Dimension::AUTO,
        };

        let mut style = TaffyStyle {
            size: TaffySize {
                width: width.unwrap_or(Dimension::AUTO),
                height: height.unwrap_or(Dimension::AUTO),
            },
            min_size: TaffySize {
                width: self
                    .layout
                    .min_width
                    .as_ref()
                    .map(|value| {
                        track_property_scope(PropertySlot::MinWidth, || {
                            resolve_dimension(
                                value,
                                animations,
                                self.id,
                                WidgetProperty::Width,
                                now,
                                units,
                            )
                        })
                    })
                    .unwrap_or(default_min_width),
                height: self
                    .layout
                    .min_height
                    .as_ref()
                    .map(|value| {
                        track_property_scope(PropertySlot::MinHeight, || {
                            resolve_dimension(
                                value,
                                animations,
                                self.id,
                                WidgetProperty::Height,
                                now,
                                units,
                            )
                        })
                    })
                    .unwrap_or(default_min_height),
            },
            max_size: TaffySize {
                width: self
                    .layout
                    .max_width
                    .as_ref()
                    .map(|value| {
                        track_property_scope(PropertySlot::MaxWidth, || {
                            resolve_dimension(
                                value,
                                animations,
                                self.id,
                                WidgetProperty::Width,
                                now,
                                units,
                            )
                        })
                    })
                    .unwrap_or(Dimension::AUTO),
                height: self
                    .layout
                    .max_height
                    .as_ref()
                    .map(|value| {
                        track_property_scope(PropertySlot::MaxHeight, || {
                            resolve_dimension(
                                value,
                                animations,
                                self.id,
                                WidgetProperty::Height,
                                now,
                                units,
                            )
                        })
                    })
                    .unwrap_or(Dimension::AUTO),
            },
            margin: to_taffy_rect_auto(
                track_property_scope(PropertySlot::Margin, || {
                    self.layout.margin.resolve_widget(
                        animations,
                        self.id,
                        WidgetProperty::Margin,
                        now,
                    )
                }),
                units,
            ),
            padding: to_taffy_rect(
                self.layout
                    .padding
                    .as_ref()
                    .map(|padding| {
                        track_property_scope(PropertySlot::Padding, || {
                            padding.resolve_widget(
                                animations,
                                self.id,
                                WidgetProperty::Padding,
                                now,
                            )
                        })
                    })
                    .unwrap_or_else(|| default_layout_padding(self, theme)),
                units,
            ),
            flex_grow: track_property_scope(PropertySlot::Grow, || {
                self.layout
                    .grow
                    .resolve_widget(animations, self.id, WidgetProperty::Grow, now)
            })
            .max(0.0),
            flex_shrink: track_property_scope(PropertySlot::Shrink, || {
                self.layout
                    .shrink
                    .resolve_widget(animations, self.id, WidgetProperty::Grow, now)
            })
            .max(0.0),
            flex_basis: self
                .layout
                .basis
                .as_ref()
                .map(|value| {
                    track_property_scope(PropertySlot::Basis, || {
                        resolve_dimension(
                            value,
                            animations,
                            self.id,
                            WidgetProperty::Width,
                            now,
                            units,
                        )
                    })
                })
                .unwrap_or(Dimension::AUTO),
            aspect_ratio: self.layout.aspect_ratio.as_ref().map(|value| {
                track_property_scope(PropertySlot::AspectRatio, || {
                    value.resolve_widget(animations, self.id, WidgetProperty::Grow, now)
                })
                .max(0.0)
            }),
            position: match self.layout.position_type {
                PositionType::Relative => TaffyPosition::Relative,
                PositionType::Absolute => TaffyPosition::Absolute,
            },
            inset: taffy::Rect {
                left: self
                    .layout
                    .left
                    .as_ref()
                    .map(|value| {
                        track_property_scope(PropertySlot::Inset, || {
                            resolve_length_percentage_auto(
                                value,
                                animations,
                                self.id,
                                WidgetProperty::Width,
                                now,
                                units,
                            )
                        })
                    })
                    .unwrap_or(LengthPercentageAuto::AUTO),
                right: self
                    .layout
                    .right
                    .as_ref()
                    .map(|value| {
                        track_property_scope(PropertySlot::Inset, || {
                            resolve_length_percentage_auto(
                                value,
                                animations,
                                self.id,
                                WidgetProperty::Width,
                                now,
                                units,
                            )
                        })
                    })
                    .unwrap_or(LengthPercentageAuto::AUTO),
                top: self
                    .layout
                    .top
                    .as_ref()
                    .map(|value| {
                        track_property_scope(PropertySlot::Inset, || {
                            resolve_length_percentage_auto(
                                value,
                                animations,
                                self.id,
                                WidgetProperty::Height,
                                now,
                                units,
                            )
                        })
                    })
                    .unwrap_or(LengthPercentageAuto::AUTO),
                bottom: self
                    .layout
                    .bottom
                    .as_ref()
                    .map(|value| {
                        track_property_scope(PropertySlot::Inset, || {
                            resolve_length_percentage_auto(
                                value,
                                animations,
                                self.id,
                                WidgetProperty::Height,
                                now,
                                units,
                            )
                        })
                    })
                    .unwrap_or(LengthPercentageAuto::AUTO),
            },
            align_self: self.layout.align_self.map(map_align_self),
            justify_self: self.layout.justify_self.map(map_align_self),
            ..Default::default()
        };

        if matches!(parent_kind, Some(ContainerKind::Stack)) {
            style.grid_row.start = line(1);
            style.grid_row.end = span(self.layout.row_span.max(1) as u16);
            style.grid_column.start = line(1);
            style.grid_column.end = span(self.layout.column_span.max(1) as u16);
        } else {
            if let Some(start) = self.layout.row_start.as_ref() {
                let start = track_property_scope(PropertySlot::GridRow, || start.resolve()).max(1);
                style.grid_row.start = line(start as i16);
            }
            if self.layout.row_span > 1 {
                style.grid_row.end = span(self.layout.row_span as u16);
            }
            if let Some(start) = self.layout.column_start.as_ref() {
                let start =
                    track_property_scope(PropertySlot::GridColumn, || start.resolve()).max(1);
                style.grid_column.start = line(start as i16);
            }
            if self.layout.column_span > 1 {
                style.grid_column.end = span(self.layout.column_span as u16);
            }
        }

        match &self.kind {
            ResolvedWidgetKind::Container { layout, .. } => {
                apply_container_style(&mut style, layout, animations, self.id, units, now);
            }
            ResolvedWidgetKind::Virtual { .. } => {
                style.display = Display::Grid;
                style.grid_template_columns =
                    vec![GridTemplateComponent::Single(TrackSizingFunction::AUTO)];
                style.grid_template_rows =
                    vec![GridTemplateComponent::Single(TrackSizingFunction::AUTO)];
            }
            ResolvedWidgetKind::ToastHost { .. } => {
                style.display = Display::None;
            }
            _ => {}
        }

        style
    }
}
