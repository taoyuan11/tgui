use crate::core::{DpiScale, ElementId, Error, Result, Size};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Identifies which subsystem supplies an intrinsic size. All kinds use the
/// same callback and cache path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MeasureKind {
    Text,
    Image,
    VirtualList,
    NativeHost,
    Custom,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KnownDimensions {
    pub width: Option<f32>,
    pub height: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AvailableDimension {
    Definite(f32),
    MinContent,
    MaxContent,
}

impl AvailableDimension {
    pub const fn definite(self) -> Option<f32> {
        match self {
            Self::Definite(value) => Some(value),
            Self::MinContent | Self::MaxContent => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AvailableSize {
    pub width: AvailableDimension,
    pub height: AvailableDimension,
}

/// Immutable input shared by Text, Image, VirtualList, NativeHost, and custom
/// intrinsic measurement providers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasureInput {
    pub known_dimensions: KnownDimensions,
    pub available_space: AvailableSize,
    pub style_fingerprint: u64,
    pub content_generation: u64,
    pub font_generation: u64,
    pub scale: DpiScale,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeasureOutput {
    pub size: Size,
    /// Baseline in logical pixels relative to the measured box's top edge.
    pub baseline: Option<f32>,
}

impl MeasureOutput {
    pub const fn new(size: Size) -> Self {
        Self {
            size,
            baseline: None,
        }
    }

    pub const fn with_baseline(mut self, baseline: f32) -> Self {
        self.baseline = Some(baseline);
        self
    }

    pub fn validate(self) -> Result<Self> {
        self.size.validate().map_err(Error::from)?;
        if let Some(baseline) = self.baseline {
            if !baseline.is_finite() || baseline < 0.0 {
                return Err(Error::invalid_input(
                    Some("measure.baseline".to_owned()),
                    "baseline must be finite and non-negative",
                ));
            }
        }
        Ok(self)
    }
}

pub trait Measure: 'static {
    fn measure(&self, input: MeasureInput) -> Result<MeasureOutput>;
}

impl<F> Measure for F
where
    F: Fn(MeasureInput) -> Result<MeasureOutput> + 'static,
{
    fn measure(&self, input: MeasureInput) -> Result<MeasureOutput> {
        self(input)
    }
}

static NEXT_MEASURE_ID: AtomicU64 = AtomicU64::new(1);

/// Cloneable, identity-stable callback handle. Cloning a handle across Widget
/// rebuilds preserves Taffy and measurement-cache identity.
#[derive(Clone)]
pub struct MeasureHandle {
    id: u64,
    revision: u64,
    kind: MeasureKind,
    callback: Rc<dyn Measure>,
}

impl MeasureHandle {
    pub fn new(kind: MeasureKind, callback: impl Measure) -> Self {
        let id = NEXT_MEASURE_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .expect("measure handle identity space exhausted");
        Self {
            id,
            revision: 0,
            kind,
            callback: Rc::new(callback),
        }
    }

    pub fn text(callback: impl Measure) -> Self {
        Self::new(MeasureKind::Text, callback)
    }

    pub fn image(callback: impl Measure) -> Self {
        Self::new(MeasureKind::Image, callback)
    }

    pub fn virtual_list(callback: impl Measure) -> Self {
        Self::new(MeasureKind::VirtualList, callback)
    }

    pub fn native_host(callback: impl Measure) -> Self {
        Self::new(MeasureKind::NativeHost, callback)
    }

    pub fn custom(callback: impl Measure) -> Self {
        Self::new(MeasureKind::Custom, callback)
    }

    /// Changes callback cache identity while retaining the stable provider ID.
    /// Use this when callback captures change observably.
    pub const fn with_revision(mut self, revision: u64) -> Self {
        self.revision = revision;
        self
    }

    pub const fn kind(&self) -> MeasureKind {
        self.kind
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn measure(&self, input: MeasureInput) -> Result<MeasureOutput> {
        self.callback.measure(input)?.validate()
    }
}

impl fmt::Debug for MeasureHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MeasureHandle")
            .field("id", &self.id)
            .field("revision", &self.revision)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl PartialEq for MeasureHandle {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.revision == other.revision && self.kind == other.kind
    }
}

impl Eq for MeasureHandle {}

/// Per-element intrinsic content metadata included in measurement-cache keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasureSpec {
    handle: MeasureHandle,
    content_generation: u64,
    font_generation: u64,
}

impl MeasureSpec {
    pub const fn new(handle: MeasureHandle) -> Self {
        Self {
            handle,
            content_generation: 0,
            font_generation: 0,
        }
    }

    pub const fn with_content_generation(mut self, generation: u64) -> Self {
        self.content_generation = generation;
        self
    }

    pub const fn with_font_generation(mut self, generation: u64) -> Self {
        self.font_generation = generation;
        self
    }

    pub const fn handle(&self) -> &MeasureHandle {
        &self.handle
    }

    pub const fn content_generation(&self) -> u64 {
        self.content_generation
    }

    pub const fn font_generation(&self) -> u64 {
        self.font_generation
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MeasureCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub entries: usize,
    pub capacity: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum OptionalFloatKey {
    None,
    Some(u32),
}

impl From<Option<f32>> for OptionalFloatKey {
    fn from(value: Option<f32>) -> Self {
        value.map_or(Self::None, |value| Self::Some(normalized_bits(value)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum AvailableKey {
    Definite(u32),
    MinContent,
    MaxContent,
}

impl From<AvailableDimension> for AvailableKey {
    fn from(value: AvailableDimension) -> Self {
        match value {
            AvailableDimension::Definite(value) => Self::Definite(normalized_bits(value)),
            AvailableDimension::MinContent => Self::MinContent,
            AvailableDimension::MaxContent => Self::MaxContent,
        }
    }
}

/// Exact cache identity required by the P2 measurement contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MeasureCacheKey {
    known_width: OptionalFloatKey,
    known_height: OptionalFloatKey,
    available_width: AvailableKey,
    available_height: AvailableKey,
    style_fingerprint: u64,
    content_generation: u64,
    font_generation: u64,
    scale: u64,
    measure_id: u64,
    measure_revision: u64,
}

impl MeasureCacheKey {
    fn new(input: MeasureInput, handle: &MeasureHandle) -> Self {
        Self {
            known_width: input.known_dimensions.width.into(),
            known_height: input.known_dimensions.height.into(),
            available_width: input.available_space.width.into(),
            available_height: input.available_space.height.into(),
            style_fingerprint: input.style_fingerprint,
            content_generation: input.content_generation,
            font_generation: input.font_generation,
            scale: normalized_f64_bits(input.scale.get()),
            measure_id: handle.id,
            measure_revision: handle.revision,
        }
    }
}

const DEFAULT_MEASURE_CACHE_CAPACITY: usize = 4_096;

#[derive(Clone, Copy, Debug)]
struct CachedMeasure {
    output: MeasureOutput,
    last_used: u64,
}

#[derive(Debug)]
pub(crate) struct MeasureCache {
    entries: HashMap<(ElementId, MeasureCacheKey), CachedMeasure>,
    capacity: usize,
    usage_clock: u64,
    hits: u64,
    misses: u64,
    evictions: u64,
}

impl MeasureCache {
    fn with_capacity(capacity: usize) -> Self {
        debug_assert!(capacity > 0, "measurement cache capacity must be non-zero");
        Self {
            entries: HashMap::new(),
            capacity: capacity.max(1),
            usage_clock: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
        }
    }

    pub(crate) fn get_or_measure(
        &mut self,
        element: ElementId,
        spec: &MeasureSpec,
        input: MeasureInput,
        run: impl FnOnce(&MeasureHandle, MeasureInput) -> Result<MeasureOutput>,
    ) -> Result<MeasureOutput> {
        let key = MeasureCacheKey::new(input, &spec.handle);
        let cache_key = (element, key);
        let last_used = self.next_usage();
        if let Some(entry) = self.entries.get_mut(&cache_key) {
            entry.last_used = last_used;
            self.hits = self.hits.saturating_add(1);
            return Ok(entry.output);
        }
        self.misses = self.misses.saturating_add(1);
        let output = run(&spec.handle, input)?.validate()?;
        if self.entries.len() >= self.capacity {
            if let Some(least_recent) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| *key)
            {
                self.entries.remove(&least_recent);
                self.evictions = self.evictions.saturating_add(1);
            }
        }
        self.entries
            .insert(cache_key, CachedMeasure { output, last_used });
        Ok(output)
    }

    pub(crate) fn invalidate(&mut self, element: ElementId) {
        self.entries
            .retain(|(candidate, _), _| *candidate != element);
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(crate) fn retain_elements(&mut self, mut retain: impl FnMut(ElementId) -> bool) {
        self.entries.retain(|(element, _), _| retain(*element));
    }

    pub(crate) fn stats(&self) -> MeasureCacheStats {
        MeasureCacheStats {
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            entries: self.entries.len(),
            capacity: self.capacity,
        }
    }

    fn next_usage(&mut self) -> u64 {
        self.usage_clock = self.usage_clock.saturating_add(1);
        self.usage_clock
    }
}

impl Default for MeasureCache {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_MEASURE_CACHE_CAPACITY)
    }
}

fn normalized_bits(value: f32) -> u32 {
    if value == 0.0 {
        0.0_f32.to_bits()
    } else {
        value.to_bits()
    }
}

fn normalized_f64_bits(value: f64) -> u64 {
    if value == 0.0 {
        0.0_f64.to_bits()
    } else {
        value.to_bits()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn cache_key_covers_style_generations_constraints_and_scale() {
        let calls = Rc::new(Cell::new(0));
        let calls_copy = calls.clone();
        let handle = MeasureHandle::text(move |_| {
            calls_copy.set(calls_copy.get() + 1);
            Ok(MeasureOutput::new(Size::new(10.0, 4.0)))
        });
        let spec = MeasureSpec::new(handle);
        let input = MeasureInput {
            known_dimensions: KnownDimensions::default(),
            available_space: AvailableSize {
                width: AvailableDimension::Definite(100.0),
                height: AvailableDimension::MaxContent,
            },
            style_fingerprint: 7,
            content_generation: 1,
            font_generation: 2,
            scale: DpiScale::ONE,
        };
        let element = ElementId::from_parts(1, 1);
        let mut cache = MeasureCache::default();
        let run = |handle: &MeasureHandle, input| handle.measure(input);
        cache.get_or_measure(element, &spec, input, run).unwrap();
        cache.get_or_measure(element, &spec, input, run).unwrap();
        let changed_scale = MeasureInput {
            scale: DpiScale::new(2.0).unwrap(),
            ..input
        };
        cache
            .get_or_measure(element, &spec, changed_scale, run)
            .unwrap();

        assert_eq!(calls.get(), 2);
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 2);
    }

    #[test]
    fn cache_has_a_hard_lru_entry_limit() {
        let calls = Rc::new(Cell::new(0));
        let calls_copy = calls.clone();
        let spec = MeasureSpec::new(MeasureHandle::text(move |_| {
            calls_copy.set(calls_copy.get() + 1);
            Ok(MeasureOutput::new(Size::new(10.0, 4.0)))
        }));
        let input = MeasureInput {
            known_dimensions: KnownDimensions::default(),
            available_space: AvailableSize {
                width: AvailableDimension::Definite(100.0),
                height: AvailableDimension::MaxContent,
            },
            style_fingerprint: 1,
            content_generation: 0,
            font_generation: 0,
            scale: DpiScale::ONE,
        };
        let element = ElementId::from_parts(1, 1);
        let mut cache = MeasureCache::with_capacity(2);
        let run = |handle: &MeasureHandle, input| handle.measure(input);

        cache.get_or_measure(element, &spec, input, run).unwrap();
        cache
            .get_or_measure(
                element,
                &spec,
                MeasureInput {
                    style_fingerprint: 2,
                    ..input
                },
                run,
            )
            .unwrap();
        // Refresh the first entry, making the second one the LRU victim.
        cache.get_or_measure(element, &spec, input, run).unwrap();
        cache
            .get_or_measure(
                element,
                &spec,
                MeasureInput {
                    style_fingerprint: 3,
                    ..input
                },
                run,
            )
            .unwrap();
        cache
            .get_or_measure(
                element,
                &spec,
                MeasureInput {
                    style_fingerprint: 2,
                    ..input
                },
                run,
            )
            .unwrap();

        assert_eq!(calls.get(), 4);
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 4);
        assert_eq!(cache.stats().evictions, 2);
        assert_eq!(cache.stats().entries, 2);
        assert_eq!(cache.stats().capacity, 2);
    }
}
