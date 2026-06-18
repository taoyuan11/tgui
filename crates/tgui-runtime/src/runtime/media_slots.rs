use super::*;
use crate::media::{MediaCompletion, MediaTextureKey, TextureFrame};
use crate::runtime::state::{
    MediaTextureBinding, MediaTextureBindingIndex, MediaTextureBindingSlot,
    ReactiveMediaTextureBindingUpdate,
};
use crate::ui::widget::{
    SceneCounts, ShapePrimitiveSlot, TextPrimitiveSlot, TexturePrimitive, TexturePrimitiveSlot,
};

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn rebuild_media_texture_bindings(&mut self) {
        let Some(cached) = self.cached_scene.as_mut() else {
            return;
        };
        let had_bindings = !cached.media_texture_bindings.is_empty()
            || !cached.media_texture_binding_index.is_empty();
        let (bindings, index) = build_media_texture_bindings(cached);
        let has_bindings = !bindings.is_empty() || !index.is_empty();
        cached.media_texture_bindings = bindings;
        cached.media_texture_binding_index = index;
        if had_bindings || has_bindings {
            super::action_stats::record("media_texture_binding_full_rebuild");
        }
    }

    pub(super) fn sync_reactive_media_texture_bindings(
        &mut self,
        updates: &[ReactiveMediaTextureBindingUpdate],
    ) -> bool {
        if updates.is_empty() {
            return true;
        }
        let Some(cached) = self.cached_scene.as_mut() else {
            return false;
        };
        for update in updates {
            if !sync_reactive_media_texture_binding(cached, update) {
                return false;
            }
        }
        true
    }

    pub(super) fn try_patch_media_completions(&mut self, completions: &[MediaCompletion]) -> bool {
        if completions.is_empty() {
            return false;
        }

        let mut patches = Vec::<MediaTexturePatch>::new();
        let mut seen_raster_keys = HashSet::new();
        let mut handled_pending_source = false;
        for completion in completions {
            match completion {
                MediaCompletion::RasterFinished { key } => {
                    if !seen_raster_keys.insert(key.clone()) {
                        continue;
                    }
                    let Some(cached) = self.cached_scene.as_ref() else {
                        return false;
                    };
                    let Some(bindings) = cached.media_texture_bindings.get(key).cloned() else {
                        return false;
                    };
                    if bindings.is_empty() {
                        return false;
                    }
                    let snapshot = self
                        .media_manager
                        .image_snapshot(&key.source, Some(key.raster_request));
                    let Some(texture) = snapshot.texture else {
                        if snapshot.loading && snapshot.error.is_none() {
                            handled_pending_source = true;
                            continue;
                        }
                        return false;
                    };
                    for binding in bindings {
                        patches.push(MediaTexturePatch {
                            old_key: key.clone(),
                            new_key: key.clone(),
                            binding,
                            texture: Some(texture.clone()),
                            frame: None,
                            hide_placeholder: true,
                        });
                    }
                }
                MediaCompletion::SourceLoaded { source } => {
                    let Some(cached) = self.cached_scene.as_ref() else {
                        return false;
                    };
                    let source_bindings = cached
                        .media_texture_bindings
                        .iter()
                        .filter(|(key, _)| &key.source == source)
                        .flat_map(|(key, bindings)| {
                            bindings
                                .iter()
                                .cloned()
                                .map(|binding| (key.clone(), binding))
                        })
                        .collect::<Vec<_>>();
                    if source_bindings.is_empty() {
                        return false;
                    }
                    for (old_key, binding) in source_bindings {
                        let Some((new_key, frame)) =
                            source_loaded_media_key(&self.media_manager, &old_key, &binding)
                        else {
                            return false;
                        };
                        let snapshot = self
                            .media_manager
                            .image_snapshot(&new_key.source, Some(new_key.raster_request));
                        if let Some(texture) = snapshot.texture {
                            patches.push(MediaTexturePatch {
                                old_key,
                                new_key,
                                binding,
                                texture: Some(texture),
                                frame,
                                hide_placeholder: true,
                            });
                        } else if snapshot.loading && snapshot.error.is_none() {
                            if old_key != new_key
                                || frame
                                    .map(|frame| {
                                        binding_current_frame(cached, &old_key, &binding)
                                            .map(|current| current != frame)
                                            .unwrap_or(true)
                                    })
                                    .unwrap_or(false)
                            {
                                patches.push(MediaTexturePatch {
                                    old_key,
                                    new_key,
                                    binding,
                                    texture: None,
                                    frame,
                                    hide_placeholder: false,
                                });
                            }
                            handled_pending_source = true;
                        } else {
                            return false;
                        }
                    }
                }
            }
        }
        if patches.is_empty() {
            return handled_pending_source;
        }

        let Some(cached) = self.cached_scene.as_mut() else {
            return false;
        };
        for patch in patches {
            if !write_media_texture_binding(cached, &patch) {
                return false;
            }
            if patch.old_key != patch.new_key {
                if !replace_media_texture_binding_key(
                    cached,
                    &patch.old_key,
                    patch.new_key,
                    patch.binding,
                ) {
                    return false;
                }
            }
        }
        cached.computed_valid = true;
        true
    }
}

fn build_media_texture_bindings<VM: 'static>(
    cached: &CachedScene<VM>,
) -> (
    HashMap<MediaTextureKey, Vec<MediaTextureBinding>>,
    HashMap<MediaTextureBindingSlot, MediaTextureBindingIndex>,
) {
    let Some(layout) = cached.layout.as_ref() else {
        return (HashMap::new(), HashMap::new());
    };
    let mut bindings: HashMap<MediaTextureKey, Vec<MediaTextureBinding>> = HashMap::new();
    let mut index: HashMap<MediaTextureBindingSlot, MediaTextureBindingIndex> = HashMap::new();
    let zero = SceneCounts::default();

    for widget_id in layout.all_widget_ids() {
        let Some(target_chunk) = cached.scene_chunks.get(&widget_id) else {
            continue;
        };
        let (local_scene, has_chunk_part) =
            if let Some(parts) = cached.scene_chunk_parts.get(&widget_id) {
                (&parts.before_children, true)
            } else {
                (target_chunk, false)
            };

        let slots = local_scene
            .scene
            .matching_texture_slots(|texture| texture.media_key.is_some());
        for slot in slots {
            let Some(texture) = local_scene.texture_slot(&zero, slot) else {
                continue;
            };
            let Some(key) = texture.media_key.clone() else {
                continue;
            };
            if !texture_slot_matches_key(target_chunk, &zero, slot, &key) {
                continue;
            }
            let (placeholder_shape_slot, placeholder_text_slot) =
                media_placeholder_slots(local_scene, media_placeholder_frame(texture));
            let candidate_binding = MediaTextureBinding {
                widget_id,
                slot,
                media_layout: texture.media_layout,
                placeholder_shape_slot,
                placeholder_text_slot,
                root_offset: SceneCounts::default(),
                ancestor_offsets: Vec::new(),
                has_chunk_part,
            };
            if !placeholder_slots_match(target_chunk, &zero, &candidate_binding) {
                continue;
            }

            let (root_offset, ancestor_offsets) = if widget_id == layout.root_id() {
                (SceneCounts::default(), Vec::new())
            } else {
                let Some(offsets) = layout.scene_splice_ancestor_offsets(
                    widget_id,
                    &cached.scene_chunk_parts,
                    &cached.scene_chunks,
                ) else {
                    continue;
                };
                let Some((root_id, root_offset, _, _)) = offsets.first().copied() else {
                    continue;
                };
                if root_id != layout.root_id() {
                    continue;
                }
                let mut ancestor_offsets = Vec::with_capacity(offsets.len());
                let mut valid = true;
                for (ancestor_id, offset, _, _) in offsets {
                    let Some(ancestor_chunk) = cached.scene_chunks.get(&ancestor_id) else {
                        valid = false;
                        break;
                    };
                    if !texture_slot_matches_key(ancestor_chunk, &offset, slot, &key) {
                        valid = false;
                        break;
                    }
                    let ancestor_binding = MediaTextureBinding {
                        root_offset: offset,
                        ancestor_offsets: Vec::new(),
                        ..candidate_binding.clone()
                    };
                    if !placeholder_slots_match(ancestor_chunk, &offset, &ancestor_binding) {
                        valid = false;
                        break;
                    }
                    ancestor_offsets.push((ancestor_id, offset));
                }
                if !valid {
                    continue;
                }
                (root_offset, ancestor_offsets)
            };

            if !texture_slot_matches_key(&cached.computed, &root_offset, slot, &key) {
                continue;
            }
            let root_binding = MediaTextureBinding {
                root_offset,
                ancestor_offsets: Vec::new(),
                ..candidate_binding.clone()
            };
            if !placeholder_slots_match(&cached.computed, &root_offset, &root_binding) {
                continue;
            }

            insert_media_texture_binding(
                &mut bindings,
                &mut index,
                key,
                MediaTextureBinding {
                    widget_id,
                    slot,
                    media_layout: texture.media_layout,
                    placeholder_shape_slot,
                    placeholder_text_slot,
                    root_offset,
                    ancestor_offsets,
                    has_chunk_part,
                },
            );
        }
    }

    (bindings, index)
}

fn sync_reactive_media_texture_binding<VM: 'static>(
    cached: &mut CachedScene<VM>,
    update: &ReactiveMediaTextureBindingUpdate,
) -> bool {
    if !remove_media_texture_binding_for_slot(cached, update.widget_id, update.slot) {
        return false;
    }
    let Some(key) = update.media_key.clone() else {
        return true;
    };

    let zero = SceneCounts::default();
    let Some(target_chunk) = cached.scene_chunks.get(&update.widget_id) else {
        return false;
    };
    let (local_scene, has_chunk_part) =
        if let Some(parts) = cached.scene_chunk_parts.get(&update.widget_id) {
            (&parts.before_children, true)
        } else {
            (target_chunk, false)
        };
    if has_chunk_part != update.has_chunk_part {
        return false;
    }
    if !texture_slot_matches_key(local_scene, &zero, update.slot, &key)
        || !texture_slot_matches_key(target_chunk, &zero, update.slot, &key)
    {
        return false;
    }

    let (placeholder_shape_slot, placeholder_text_slot) = media_placeholder_slots(
        local_scene,
        update
            .media_layout
            .map(|layout| layout.content_frame)
            .unwrap_or(update.frame),
    );
    let binding = MediaTextureBinding {
        widget_id: update.widget_id,
        slot: update.slot,
        media_layout: update.media_layout,
        placeholder_shape_slot,
        placeholder_text_slot,
        root_offset: update.root_offset,
        ancestor_offsets: update.ancestor_offsets.clone(),
        has_chunk_part: update.has_chunk_part,
    };
    if !media_texture_binding_matches(cached, &key, &binding) {
        return false;
    }
    insert_media_texture_binding(
        &mut cached.media_texture_bindings,
        &mut cached.media_texture_binding_index,
        key,
        binding,
    );
    true
}

fn media_texture_binding_matches<VM: 'static>(
    cached: &CachedScene<VM>,
    key: &MediaTextureKey,
    binding: &MediaTextureBinding,
) -> bool {
    let zero = SceneCounts::default();
    if !cached
        .scene_chunks
        .get(&binding.widget_id)
        .map(|chunk| {
            texture_slot_matches_key(chunk, &zero, binding.slot, key)
                && placeholder_slots_match(chunk, &zero, binding)
        })
        .unwrap_or(false)
    {
        return false;
    }
    for (ancestor_id, offset) in &binding.ancestor_offsets {
        let Some(ancestor_chunk) = cached.scene_chunks.get(ancestor_id) else {
            return false;
        };
        if !texture_slot_matches_key(ancestor_chunk, offset, binding.slot, key)
            || !placeholder_slots_match(ancestor_chunk, offset, binding)
        {
            return false;
        }
    }
    texture_slot_matches_key(&cached.computed, &binding.root_offset, binding.slot, key)
        && placeholder_slots_match(&cached.computed, &binding.root_offset, binding)
}

fn insert_media_texture_binding(
    bindings: &mut HashMap<MediaTextureKey, Vec<MediaTextureBinding>>,
    index: &mut HashMap<MediaTextureBindingSlot, MediaTextureBindingIndex>,
    key: MediaTextureKey,
    binding: MediaTextureBinding,
) {
    let slot = MediaTextureBindingSlot::new(binding.widget_id, binding.slot);
    let entries = bindings.entry(key.clone()).or_default();
    let binding_index = entries.len();
    entries.push(binding);
    index.insert(
        slot,
        MediaTextureBindingIndex {
            key,
            index: binding_index,
        },
    );
}

fn remove_media_texture_binding_for_slot<VM: 'static>(
    cached: &mut CachedScene<VM>,
    widget_id: WidgetId,
    slot: TexturePrimitiveSlot,
) -> bool {
    let slot = MediaTextureBindingSlot::new(widget_id, slot);
    let Some(binding_index) = cached.media_texture_binding_index.remove(&slot) else {
        return true;
    };
    let remove_key = {
        let Some(bindings) = cached.media_texture_bindings.get_mut(&binding_index.key) else {
            return false;
        };
        if binding_index.index >= bindings.len() {
            return false;
        }
        bindings.swap_remove(binding_index.index);
        if binding_index.index < bindings.len() {
            let moved = &bindings[binding_index.index];
            cached.media_texture_binding_index.insert(
                MediaTextureBindingSlot::new(moved.widget_id, moved.slot),
                MediaTextureBindingIndex {
                    key: binding_index.key.clone(),
                    index: binding_index.index,
                },
            );
        }
        bindings.is_empty()
    };
    if remove_key {
        cached.media_texture_bindings.remove(&binding_index.key);
    }
    true
}

fn replace_media_texture_binding_key<VM: 'static>(
    cached: &mut CachedScene<VM>,
    old_key: &MediaTextureKey,
    new_key: MediaTextureKey,
    binding: MediaTextureBinding,
) -> bool {
    let slot = MediaTextureBindingSlot::new(binding.widget_id, binding.slot);
    let Some(index) = cached.media_texture_binding_index.get(&slot) else {
        return false;
    };
    if &index.key != old_key {
        return false;
    }
    if !remove_media_texture_binding_for_slot(cached, binding.widget_id, binding.slot) {
        return false;
    }
    if !media_texture_binding_matches(cached, &new_key, &binding) {
        return false;
    }
    insert_media_texture_binding(
        &mut cached.media_texture_bindings,
        &mut cached.media_texture_binding_index,
        new_key,
        binding,
    );
    true
}

fn media_placeholder_slots<VM>(
    computed: &ComputedScene<VM>,
    texture_frame: Rect,
) -> (Option<ShapePrimitiveSlot>, Option<TextPrimitiveSlot>) {
    let text_slots = computed.scene.matching_text_slots(|text| {
        text.content.starts_with("loading ")
            || text.content.ends_with(" unavailable")
            || text.content.contains(" error:")
    });
    if text_slots.len() != 1 {
        return (None, None);
    }
    let shape_slots = computed
        .scene
        .matching_shape_slots(|shape| shape.stroke_width == 0.0 && shape.rect == texture_frame);
    if shape_slots.len() == 1 {
        (Some(shape_slots[0]), Some(text_slots[0]))
    } else {
        (None, Some(text_slots[0]))
    }
}

fn media_placeholder_frame(texture: &TexturePrimitive) -> Rect {
    texture
        .media_layout
        .map(|layout| layout.content_frame)
        .unwrap_or(texture.frame)
}

fn texture_slot_matches_key<VM>(
    computed: &ComputedScene<VM>,
    offset: &SceneCounts,
    slot: TexturePrimitiveSlot,
    key: &MediaTextureKey,
) -> bool {
    computed
        .texture_slot(offset, slot)
        .and_then(|texture| texture.media_key.as_ref())
        == Some(key)
}

fn placeholder_slots_match<VM>(
    computed: &ComputedScene<VM>,
    offset: &SceneCounts,
    binding: &MediaTextureBinding,
) -> bool {
    binding
        .placeholder_shape_slot
        .map(|slot| computed.can_write_shape_color_slot(offset, slot))
        .unwrap_or(true)
        && binding
            .placeholder_text_slot
            .map(|slot| computed.can_write_text_color_slot(offset, slot))
            .unwrap_or(true)
}

#[derive(Clone)]
struct MediaTexturePatch {
    old_key: MediaTextureKey,
    new_key: MediaTextureKey,
    binding: MediaTextureBinding,
    texture: Option<Arc<TextureFrame>>,
    frame: Option<Rect>,
    hide_placeholder: bool,
}

fn source_loaded_media_key(
    media_manager: &MediaManager,
    old_key: &MediaTextureKey,
    binding: &MediaTextureBinding,
) -> Option<(MediaTextureKey, Option<Rect>)> {
    let Some(layout) = binding.media_layout else {
        return Some((old_key.clone(), None));
    };
    let metadata = media_manager.image_snapshot(&old_key.source, None);
    if metadata.error.is_some() {
        return None;
    }
    layout
        .texture_key(old_key.source.clone(), metadata.intrinsic_size)
        .map(|(key, frame)| (key, Some(frame)))
}

fn binding_current_frame<VM>(
    cached: &CachedScene<VM>,
    key: &MediaTextureKey,
    binding: &MediaTextureBinding,
) -> Option<Rect> {
    let zero = SceneCounts::default();
    let texture = cached
        .scene_chunks
        .get(&binding.widget_id)?
        .texture_slot(&zero, binding.slot)?;
    (texture.media_key.as_ref() == Some(key)).then_some(texture.frame)
}

fn replacement_texture_primitive<VM>(
    computed: &ComputedScene<VM>,
    offset: &SceneCounts,
    slot: TexturePrimitiveSlot,
    patch: &MediaTexturePatch,
) -> Option<TexturePrimitive> {
    let mut primitive = computed.texture_slot(offset, slot)?.clone();
    if primitive.media_key.as_ref() != Some(&patch.old_key) {
        return None;
    }
    if let Some(texture) = &patch.texture {
        primitive.texture = texture.clone();
    }
    primitive.media_key = Some(patch.new_key.clone());
    primitive.media_layout = patch.binding.media_layout.or(primitive.media_layout);
    if let Some(frame) = patch.frame {
        primitive.frame = frame;
    }
    Some(primitive)
}

fn write_media_texture_binding<VM>(
    cached: &mut CachedScene<VM>,
    patch: &MediaTexturePatch,
) -> bool {
    let binding = &patch.binding;
    let zero = SceneCounts::default();
    if !cached
        .scene_chunks
        .get(&binding.widget_id)
        .map(|chunk| placeholder_slots_match(chunk, &zero, binding))
        .unwrap_or(false)
    {
        return false;
    }
    let Some(primitive) = cached
        .scene_chunks
        .get(&binding.widget_id)
        .and_then(|chunk| replacement_texture_primitive(chunk, &zero, binding.slot, patch))
    else {
        return false;
    };

    if binding.has_chunk_part {
        let Some(parts) = cached.scene_chunk_parts.get_mut(&binding.widget_id) else {
            return false;
        };
        if !parts
            .before_children
            .write_texture_slot(&zero, binding.slot, primitive.clone())
        {
            return false;
        }
        if patch.hide_placeholder
            && !write_media_placeholder_slots(&mut parts.before_children, &zero, binding)
        {
            return false;
        }
    }

    let Some(target_chunk) = cached.scene_chunks.get_mut(&binding.widget_id) else {
        return false;
    };
    if !target_chunk.write_texture_slot(&zero, binding.slot, primitive.clone()) {
        return false;
    }
    if patch.hide_placeholder && !write_media_placeholder_slots(target_chunk, &zero, binding) {
        return false;
    }

    for (ancestor_id, offset) in &binding.ancestor_offsets {
        let Some(ancestor_chunk) = cached.scene_chunks.get_mut(ancestor_id) else {
            return false;
        };
        if !ancestor_chunk.write_texture_slot(offset, binding.slot, primitive.clone()) {
            return false;
        }
        if patch.hide_placeholder && !write_media_placeholder_slots(ancestor_chunk, offset, binding)
        {
            return false;
        }
    }

    if !cached
        .computed
        .write_texture_slot(&binding.root_offset, binding.slot, primitive)
    {
        return false;
    }
    !patch.hide_placeholder
        || write_media_placeholder_slots(&mut cached.computed, &binding.root_offset, binding)
}

fn write_media_placeholder_slots<VM>(
    computed: &mut ComputedScene<VM>,
    offset: &SceneCounts,
    binding: &MediaTextureBinding,
) -> bool {
    if let Some(slot) = binding.placeholder_shape_slot {
        if !write_shape_slot_alpha_zero(computed, offset, slot) {
            return false;
        }
    }
    if let Some(slot) = binding.placeholder_text_slot {
        if !write_text_slot_alpha_zero(computed, offset, slot) {
            return false;
        }
    }
    true
}

fn write_shape_slot_alpha_zero<VM>(
    computed: &mut ComputedScene<VM>,
    offset: &SceneCounts,
    slot: ShapePrimitiveSlot,
) -> bool {
    let Some(shape_index) = offset.shapes.checked_add(slot.shape_index) else {
        return false;
    };
    let Some(color) = computed
        .scene
        .shapes
        .get(shape_index)
        .map(|shape| shape.color.with_alpha_factor(0.0))
    else {
        return false;
    };
    computed.write_shape_color_slot(offset, slot, color)
}

fn write_text_slot_alpha_zero<VM>(
    computed: &mut ComputedScene<VM>,
    offset: &SceneCounts,
    slot: TextPrimitiveSlot,
) -> bool {
    let Some(text_index) = offset.texts.checked_add(slot.text_index) else {
        return false;
    };
    let Some(color) = computed
        .scene
        .texts
        .get(text_index)
        .map(|text| text.color.with_alpha_factor(0.0))
    else {
        return false;
    };
    computed.write_text_color_slot(offset, slot, color)
}
