//! Native-host escape hatch for capabilities the retained paint pipeline cannot express.
//!
//! A host is owned by [`NativeHostManager`] on the UI thread. It receives immutable
//! geometry/input values and can only return [`NativeHostOutput`] values for the normal
//! event/transaction pipeline; no Widget or Element mutation API crosses this boundary.

use crate::accessibility::{NativeSemanticsBridge, Role, Semantics};
use crate::core::{
    Clip, DenseArena, DpiScale, ElementId, Error, PropertyId, Rect, ResourceId, Result, Size,
    Transform2D, WidgetKey, WindowId,
};
use crate::event::{EventKind, UiEvent};
use crate::layout::{LayoutStyle, MeasureInput, MeasureOutput};
use crate::render::{LayerSpec, PaintCommand};
use crate::state::{UiCommand, UiThread};
use crate::widget::{BuildContext, PropertyImpact, Widget, WidgetNode};
use std::sync::{Arc, Mutex};

pub use crate::core::HostHandle;

/// Reviewable invariant used by architecture tests and documentation.
pub const ORDINARY_CONTROLS_MAY_USE_NATIVE_HOST: bool = false;

pub const WEBVIEW_ADAPTER_ENABLED: bool = cfg!(feature = "webview");

pub const HOST_SURFACE_SLOT: PropertyId = PropertyId::new(u64::MAX - 100);
pub const HOST_SURFACE_GENERATION: PropertyId = PropertyId::new(u64::MAX - 101);
pub const HOST_OFFSCREEN: PropertyId = PropertyId::new(u64::MAX - 102);
pub const HOST_OPAQUE: PropertyId = PropertyId::new(u64::MAX - 103);
pub const HOST_Z_ORDER: PropertyId = PropertyId::new(u64::MAX - 104);

/// Dedicated declaration that inserts one validated native composition into
/// the normal Element -> Layout -> Paint IR -> Compiler path.
#[derive(Clone, Debug)]
pub struct NativeHostWidget {
    key: Option<WidgetKey>,
    composition: Option<NativeHostComposition>,
    z_order: i32,
    style: LayoutStyle,
    semantics: Semantics,
}

impl NativeHostWidget {
    pub fn new() -> Self {
        Self {
            key: None,
            composition: None,
            z_order: 0,
            style: LayoutStyle::default(),
            semantics: Semantics::new(Role::NativeHost)
                .with_native_bridge(NativeSemanticsBridge::Opaque),
        }
    }

    pub fn with_key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub const fn with_composition(mut self, composition: NativeHostComposition) -> Self {
        self.composition = Some(composition);
        self
    }

    pub const fn with_z_order(mut self, z_order: i32) -> Self {
        self.z_order = z_order;
        self
    }

    pub fn with_layout_style(mut self, style: LayoutStyle) -> Self {
        self.style = style;
        self
    }

    pub fn with_semantics(mut self, semantics: Semantics) -> Self {
        self.semantics = semantics;
        self
    }
}

impl Default for NativeHostWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for NativeHostWidget {
    fn build(&self, _context: &mut BuildContext) -> Result<WidgetNode> {
        let mut node = WidgetNode::new::<Self>()
            .with_optional_key(self.key.clone())
            .with_layout_style(self.style.clone())
            .with_semantics(self.semantics.clone())
            .with_property(HOST_Z_ORDER, i64::from(self.z_order))
            .with_property_impact(HOST_Z_ORDER, PropertyImpact::PAINT);
        if let Some(composition) = self.composition {
            node = node
                .with_property(HOST_SURFACE_SLOT, u64::from(composition.surface.slot()))
                .with_property(
                    HOST_SURFACE_GENERATION,
                    u64::from(composition.surface.generation()),
                )
                .with_property(
                    HOST_OFFSCREEN,
                    composition.strategy == NativeCompositionStrategy::OffscreenTexture,
                )
                .with_property(HOST_OPAQUE, composition.opaque)
                .with_property_impact(HOST_SURFACE_SLOT, PropertyImpact::RESOURCE)
                .with_property_impact(HOST_SURFACE_GENERATION, PropertyImpact::RESOURCE)
                .with_property_impact(HOST_OFFSCREEN, PropertyImpact::PAINT)
                .with_property_impact(HOST_OPAQUE, PropertyImpact::PAINT);
        }
        Ok(node)
    }
}

/// Immutable identity supplied while a platform host is created.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeHostCreateContext {
    pub window: WindowId,
    pub element: ElementId,
}

impl NativeHostCreateContext {
    pub const fn new(window: WindowId, element: ElementId) -> Self {
        Self { window, element }
    }

    fn validate(self) -> Result<()> {
        if !self.window.is_well_formed() || !self.element.is_well_formed() {
            return Err(Error::invalid_input(
                Some("native_host.context".to_owned()),
                "window and element must have non-zero generations",
            ));
        }
        Ok(())
    }
}

/// Native composition and input abilities. Every exceptional ability is opt-in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeHostCapabilities {
    pub requires_independent_surface: bool,
    pub supports_offscreen: bool,
    pub supports_transform: bool,
    pub supports_alpha: bool,
    pub supports_clip: bool,
    pub forwards_pointer: bool,
    pub forwards_keyboard: bool,
    pub forwards_ime: bool,
    pub merges_with_render_batches: bool,
}

impl NativeHostCapabilities {
    pub const fn independent_surface() -> Self {
        Self {
            requires_independent_surface: true,
            supports_offscreen: false,
            supports_transform: false,
            supports_alpha: false,
            supports_clip: false,
            forwards_pointer: true,
            forwards_keyboard: true,
            forwards_ime: true,
            merges_with_render_batches: false,
        }
    }

    pub const fn offscreen() -> Self {
        Self {
            requires_independent_surface: false,
            supports_offscreen: true,
            supports_transform: true,
            supports_alpha: true,
            supports_clip: true,
            forwards_pointer: true,
            forwards_keyboard: true,
            forwards_ime: true,
            merges_with_render_batches: true,
        }
    }

    pub fn validate(self) -> Result<Self> {
        if self.forwards_ime && !self.forwards_keyboard {
            return Err(Error::invalid_input(
                Some("native_host.capabilities.forwards_ime".to_owned()),
                "IME forwarding requires keyboard forwarding",
            ));
        }
        if self.merges_with_render_batches && !self.supports_offscreen {
            return Err(Error::invalid_input(
                Some("native_host.capabilities.merges_with_render_batches".to_owned()),
                "batch merging requires offscreen composition",
            ));
        }
        Ok(self)
    }
}

impl Default for NativeHostCapabilities {
    fn default() -> Self {
        Self::offscreen()
    }
}

/// Estimated platform/compositor cost for one host in one frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeHostCost {
    pub independent_passes: u32,
    pub surfaces: u32,
    pub texture_bytes: u64,
    pub synchronization_points: u32,
}

impl NativeHostCost {
    pub const fn new(
        independent_passes: u32,
        surfaces: u32,
        texture_bytes: u64,
        synchronization_points: u32,
    ) -> Self {
        Self {
            independent_passes,
            surfaces,
            texture_bytes,
            synchronization_points,
        }
    }

    pub const fn saturating_add(self, other: Self) -> Self {
        Self {
            independent_passes: self
                .independent_passes
                .saturating_add(other.independent_passes),
            surfaces: self.surfaces.saturating_add(other.surfaces),
            texture_bytes: self.texture_bytes.saturating_add(other.texture_bytes),
            synchronization_points: self
                .synchronization_points
                .saturating_add(other.synchronization_points),
        }
    }
}

/// Complete logical-pixel layout input committed to a mounted host.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeHostLayout {
    pub rect: Rect,
    pub dpi_scale: DpiScale,
    pub visible: bool,
    pub z_order: i32,
    pub transform: Transform2D,
    pub opacity: f32,
    pub clip: Option<Rect>,
}

impl NativeHostLayout {
    pub const fn new(rect: Rect, dpi_scale: DpiScale) -> Self {
        Self {
            rect,
            dpi_scale,
            visible: true,
            z_order: 0,
            transform: Transform2D::IDENTITY,
            opacity: 1.0,
            clip: None,
        }
    }

    pub const fn with_visibility(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub const fn with_z_order(mut self, z_order: i32) -> Self {
        self.z_order = z_order;
        self
    }

    pub const fn with_transform(mut self, transform: Transform2D) -> Self {
        self.transform = transform;
        self
    }

    pub const fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub const fn with_clip(mut self, clip: Option<Rect>) -> Self {
        self.clip = clip;
        self
    }

    pub fn validate(self) -> Result<Self> {
        self.rect.validate().map_err(Error::from)?;
        DpiScale::new(self.dpi_scale.get()).map_err(Error::from)?;
        self.transform.validate().map_err(Error::from)?;
        if let Some(clip) = self.clip {
            clip.validate().map_err(Error::from)?;
        }
        if !self.opacity.is_finite() || !(0.0..=1.0).contains(&self.opacity) {
            return Err(Error::invalid_input(
                Some("native_host.layout.opacity".to_owned()),
                "opacity must be finite and between zero and one",
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeCompositionStrategy {
    IndependentSurface,
    OffscreenTexture,
}

/// Platform resource returned in response to a scheduler composition request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeHostComposition {
    pub strategy: NativeCompositionStrategy,
    pub surface: ResourceId,
    pub opaque: bool,
}

impl NativeHostComposition {
    pub const fn new(
        strategy: NativeCompositionStrategy,
        surface: ResourceId,
        opaque: bool,
    ) -> Self {
        Self {
            strategy,
            surface,
            opaque,
        }
    }

    fn validate(self) -> Result<Self> {
        if !self.surface.is_well_formed() {
            return Err(Error::platform(
                "native_host_composition",
                "host returned a malformed surface generation",
                true,
            ));
        }
        Ok(self)
    }
}

/// The only application-facing outputs a host can produce.
#[derive(Clone, Debug, PartialEq)]
pub enum NativeHostOutput {
    Event(UiEvent),
    Command(UiCommand),
}

/// Host output stamped with both native and retained-tree generations.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHostMessage {
    pub host: HostHandle,
    pub target: ElementId,
    pub window: WindowId,
    pub output: NativeHostOutput,
}

/// Platform implementation contract. Implementors never receive a Widget/Element mutator.
pub trait NativeHost: 'static {
    fn capabilities(&self) -> NativeHostCapabilities;
    fn cost(&self) -> NativeHostCost;

    fn measure(&self, input: MeasureInput) -> Result<MeasureOutput>;

    fn mount(&mut self) -> Result<Vec<NativeHostOutput>> {
        Ok(Vec::new())
    }

    fn update_layout(&mut self, _layout: NativeHostLayout) -> Result<Vec<NativeHostOutput>> {
        Ok(Vec::new())
    }

    fn set_focus(&mut self, _focused: bool) -> Result<Vec<NativeHostOutput>> {
        Ok(Vec::new())
    }

    fn forward_input(&mut self, _event: &UiEvent) -> Result<Vec<NativeHostOutput>> {
        Ok(Vec::new())
    }

    fn compose(&mut self, strategy: NativeCompositionStrategy) -> Result<NativeHostComposition>;

    fn unmount(&mut self) -> Result<Vec<NativeHostOutput>> {
        Ok(Vec::new())
    }

    fn destroy(&mut self) -> Result<()> {
        Ok(())
    }
}

pub trait NativeHostFactory: 'static {
    fn create(&self, context: NativeHostCreateContext) -> Result<Box<dyn NativeHost>>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeHostLifecycle {
    Created,
    Mounted,
    Unmounted,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHostErrorState {
    pub operation: &'static str,
    pub message: String,
    pub recoverable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHostStatus {
    pub lifecycle: NativeHostLifecycle,
    pub focused: bool,
    pub last_error: Option<NativeHostErrorState>,
}

struct HostEntry {
    context: NativeHostCreateContext,
    host: Box<dyn NativeHost>,
    capabilities: NativeHostCapabilities,
    cost: NativeHostCost,
    status: NativeHostStatus,
    layout: Option<NativeHostLayout>,
}

impl HostEntry {
    fn fail(&mut self, operation: &'static str, error: &Error) {
        self.status.lifecycle = NativeHostLifecycle::Failed;
        self.status.focused = false;
        self.status.last_error = Some(NativeHostErrorState {
            operation,
            message: error.to_string(),
            recoverable: error.is_recoverable(),
        });
    }
}

/// UI-owned generational lifecycle manager for all native hosts in one application.
pub struct NativeHostManager {
    owner: UiThread,
    hosts: DenseArena<HostEntry, HostHandle>,
}

impl NativeHostManager {
    pub fn new() -> Self {
        Self {
            owner: UiThread::current(),
            hosts: DenseArena::new(),
        }
    }

    pub fn create(
        &mut self,
        factory: &dyn NativeHostFactory,
        context: NativeHostCreateContext,
    ) -> Result<HostHandle> {
        self.owner.assert_current()?;
        context.validate()?;
        let mut host = factory.create(context)?;
        let capabilities = match host.capabilities().validate() {
            Ok(capabilities) => capabilities,
            Err(error) => {
                let _ = host.destroy();
                return Err(error);
            }
        };
        let cost = host.cost();
        Ok(self.hosts.insert(HostEntry {
            context,
            host,
            capabilities,
            cost,
            status: NativeHostStatus {
                lifecycle: NativeHostLifecycle::Created,
                focused: false,
                last_error: None,
            },
            layout: None,
        }))
    }

    pub fn contains(&self, handle: HostHandle) -> bool {
        self.hosts.contains(handle)
    }

    pub fn len(&self) -> usize {
        self.hosts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hosts.is_empty()
    }

    pub fn status(&self, handle: HostHandle) -> Option<&NativeHostStatus> {
        self.hosts.get(handle).map(|entry| &entry.status)
    }

    pub fn capabilities(&self, handle: HostHandle) -> Option<NativeHostCapabilities> {
        self.hosts.get(handle).map(|entry| entry.capabilities)
    }

    pub fn cost(&self, handle: HostHandle) -> Option<NativeHostCost> {
        self.hosts.get(handle).map(|entry| entry.cost)
    }

    pub fn layout(&self, handle: HostHandle) -> Option<NativeHostLayout> {
        self.hosts.get(handle).and_then(|entry| entry.layout)
    }

    pub fn matches_target(&self, handle: HostHandle, window: WindowId, target: ElementId) -> bool {
        self.hosts
            .get(handle)
            .is_some_and(|entry| entry.context.window == window && entry.context.element == target)
    }

    pub fn measure(&self, handle: HostHandle, input: MeasureInput) -> Result<MeasureOutput> {
        self.owner.assert_current()?;
        let entry = self.entry(handle)?;
        entry.host.measure(input)?.validate()
    }

    pub fn mount(&mut self, handle: HostHandle) -> Result<Vec<NativeHostMessage>> {
        self.owner.assert_current()?;
        let entry = self.entry_mut(handle)?;
        if !matches!(
            entry.status.lifecycle,
            NativeHostLifecycle::Created | NativeHostLifecycle::Unmounted
        ) {
            return Err(lifecycle_error("mount", entry.status.lifecycle));
        }
        match entry.host.mount() {
            Ok(outputs) => {
                entry.status.lifecycle = NativeHostLifecycle::Mounted;
                entry.status.last_error = None;
                Ok(stamp_outputs(handle, entry.context, outputs))
            }
            Err(error) => {
                entry.fail("mount", &error);
                Err(error)
            }
        }
    }

    pub fn update_layout(
        &mut self,
        handle: HostHandle,
        layout: NativeHostLayout,
    ) -> Result<Vec<NativeHostMessage>> {
        self.owner.assert_current()?;
        let layout = layout.validate()?;
        let entry = self.mounted_entry_mut(handle, "update_layout")?;
        match entry.host.update_layout(layout) {
            Ok(outputs) => {
                entry.layout = Some(layout);
                Ok(stamp_outputs(handle, entry.context, outputs))
            }
            Err(error) => {
                entry.fail("update_layout", &error);
                Err(error)
            }
        }
    }

    pub fn set_focus(
        &mut self,
        handle: HostHandle,
        focused: bool,
    ) -> Result<Vec<NativeHostMessage>> {
        self.owner.assert_current()?;
        let entry = self.mounted_entry_mut(handle, "set_focus")?;
        match entry.host.set_focus(focused) {
            Ok(outputs) => {
                entry.status.focused = focused;
                Ok(stamp_outputs(handle, entry.context, outputs))
            }
            Err(error) => {
                entry.fail("set_focus", &error);
                Err(error)
            }
        }
    }

    pub fn forward_input(
        &mut self,
        handle: HostHandle,
        event: &UiEvent,
    ) -> Result<Vec<NativeHostMessage>> {
        self.owner.assert_current()?;
        let entry = self.mounted_entry_mut(handle, "forward_input")?;
        validate_input_capability(entry.capabilities, event)?;
        match entry.host.forward_input(event) {
            Ok(outputs) => Ok(stamp_outputs(handle, entry.context, outputs)),
            Err(error) => {
                entry.fail("forward_input", &error);
                Err(error)
            }
        }
    }

    pub fn compose(
        &mut self,
        handle: HostHandle,
        strategy: NativeCompositionStrategy,
    ) -> Result<NativeHostComposition> {
        self.owner.assert_current()?;
        let entry = self.mounted_entry_mut(handle, "compose")?;
        if strategy == NativeCompositionStrategy::OffscreenTexture
            && !entry.capabilities.supports_offscreen
        {
            return Err(Error::compile(
                "native_host_scheduler",
                "host does not support requested offscreen composition",
            ));
        }
        match entry
            .host
            .compose(strategy)
            .and_then(|value| value.validate())
        {
            Ok(composition) if composition.strategy == strategy => Ok(composition),
            Ok(_) => {
                let error = Error::platform(
                    "native_host_compose",
                    "host returned a different composition strategy than requested",
                    true,
                );
                entry.fail("compose", &error);
                Err(error)
            }
            Err(error) => {
                entry.fail("compose", &error);
                Err(error)
            }
        }
    }

    pub fn unmount(&mut self, handle: HostHandle) -> Result<Vec<NativeHostMessage>> {
        self.owner.assert_current()?;
        let entry = self.mounted_entry_mut(handle, "unmount")?;
        match entry.host.unmount() {
            Ok(outputs) => {
                entry.status.lifecycle = NativeHostLifecycle::Unmounted;
                entry.status.focused = false;
                entry.status.last_error = None;
                Ok(stamp_outputs(handle, entry.context, outputs))
            }
            Err(error) => {
                entry.fail("unmount", &error);
                Err(error)
            }
        }
    }

    /// Destroys a host and invalidates its generation. Mounted hosts are unmounted first.
    pub fn destroy(&mut self, handle: HostHandle) -> Result<Vec<NativeHostMessage>> {
        self.owner.assert_current()?;
        let mut entry = self.hosts.remove(handle).ok_or_else(stale_handle_error)?;
        let mut outputs = Vec::new();
        let mut first_error = None;
        if entry.status.lifecycle == NativeHostLifecycle::Mounted {
            match entry.host.unmount() {
                Ok(unmounted) => outputs.extend(stamp_outputs(handle, entry.context, unmounted)),
                Err(error) => first_error = Some(error),
            }
        }
        if let Err(error) = entry.host.destroy() {
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
        if let Some(error) = first_error {
            Err(error)
        } else {
            Ok(outputs)
        }
    }

    fn entry(&self, handle: HostHandle) -> Result<&HostEntry> {
        self.hosts.get(handle).ok_or_else(stale_handle_error)
    }

    fn entry_mut(&mut self, handle: HostHandle) -> Result<&mut HostEntry> {
        self.hosts.get_mut(handle).ok_or_else(stale_handle_error)
    }

    fn mounted_entry_mut(
        &mut self,
        handle: HostHandle,
        operation: &'static str,
    ) -> Result<&mut HostEntry> {
        let entry = self.entry_mut(handle)?;
        if entry.status.lifecycle != NativeHostLifecycle::Mounted {
            return Err(lifecycle_error(operation, entry.status.lifecycle));
        }
        Ok(entry)
    }
}

impl Default for NativeHostManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for NativeHostManager {
    fn drop(&mut self) {
        for (_, entry) in self.hosts.iter_mut() {
            if entry.status.lifecycle == NativeHostLifecycle::Mounted {
                let _ = entry.host.unmount();
            }
            let _ = entry.host.destroy();
        }
    }
}

fn stale_handle_error() -> Error {
    Error::platform(
        "native_host_lookup",
        "host handle is stale or belongs to another manager",
        false,
    )
}

fn lifecycle_error(operation: &'static str, state: NativeHostLifecycle) -> Error {
    Error::platform(
        operation,
        format!("operation is invalid while host is {state:?}"),
        false,
    )
}

fn stamp_outputs(
    host: HostHandle,
    context: NativeHostCreateContext,
    outputs: Vec<NativeHostOutput>,
) -> Vec<NativeHostMessage> {
    outputs
        .into_iter()
        .map(|output| NativeHostMessage {
            host,
            target: context.element,
            window: context.window,
            output,
        })
        .collect()
}

fn validate_input_capability(capabilities: NativeHostCapabilities, event: &UiEvent) -> Result<()> {
    let supported = match event.kind() {
        EventKind::PointerDown
        | EventKind::PointerMove
        | EventKind::PointerUp
        | EventKind::PointerCancel
        | EventKind::PointerEnter
        | EventKind::PointerLeave
        | EventKind::Wheel
        | EventKind::DragStart
        | EventKind::DragMove
        | EventKind::DragEnd
        | EventKind::DragCancel => capabilities.forwards_pointer,
        EventKind::KeyDown | EventKind::KeyUp | EventKind::Shortcut | EventKind::TextInput => {
            capabilities.forwards_keyboard
        }
        EventKind::Ime => capabilities.forwards_ime,
        EventKind::FocusGained | EventKind::FocusLost => false,
        EventKind::WindowResized
        | EventKind::WindowDpiChanged
        | EventKind::WindowActivated
        | EventKind::WindowCloseRequested
        | EventKind::AccessibilityAction => false,
    };
    if supported {
        Ok(())
    } else {
        Err(Error::platform(
            "native_host_input",
            format!(
                "host capability does not allow {:?} forwarding",
                event.kind()
            ),
            true,
        ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeIsolationReason {
    Transform,
    Alpha,
    Clip,
}

/// Backend-neutral scheduler decision consumed before recording a NativeSurface command.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeHostSchedule {
    pub strategy: NativeCompositionStrategy,
    pub visible: bool,
    pub pass_boundary: bool,
    pub z_order: i32,
    pub isolation_reasons: Vec<NativeIsolationReason>,
    pub cost: NativeHostCost,
    layout: NativeHostLayout,
}

impl NativeHostSchedule {
    pub const fn layout(&self) -> NativeHostLayout {
        self.layout
    }

    /// Records the host surface plus renderer-owned isolation for effects the
    /// native implementation cannot apply itself.
    pub fn paint_commands(
        &self,
        layout: NativeHostLayout,
        composition: NativeHostComposition,
    ) -> Result<Vec<PaintCommand>> {
        let layout = layout.validate()?;
        if layout != self.layout {
            return Err(Error::compile(
                "native_host_scheduler",
                "layout changed after the host composition decision was scheduled",
            ));
        }
        if !self.visible || !layout.visible {
            return Ok(Vec::new());
        }
        if self.strategy != composition.strategy {
            return Err(Error::compile(
                "native_host_scheduler",
                "composition result does not match the scheduled strategy",
            ));
        }
        composition.validate()?;

        let isolates_transform = self
            .isolation_reasons
            .contains(&NativeIsolationReason::Transform);
        let isolates_alpha = self
            .isolation_reasons
            .contains(&NativeIsolationReason::Alpha);
        let isolates_clip = self
            .isolation_reasons
            .contains(&NativeIsolationReason::Clip);
        let mut commands = Vec::with_capacity(
            1 + usize::from(isolates_transform) * 2
                + usize::from(isolates_alpha) * 2
                + usize::from(isolates_clip) * 2,
        );
        if isolates_transform {
            commands.push(PaintCommand::PushTransform(layout.transform));
        }
        if isolates_clip {
            commands.push(PaintCommand::PushClip(Clip::Rect(
                layout.clip.expect("clip isolation requires a clip"),
            )));
        }
        if isolates_alpha {
            commands.push(PaintCommand::BeginLayer(
                LayerSpec::new(layout.rect).with_opacity(layout.opacity),
            ));
        }
        commands.push(PaintCommand::NativeSurface {
            rect: layout.rect,
            surface: composition.surface,
            opaque: composition.opaque && !isolates_alpha,
        });
        if isolates_alpha {
            commands.push(PaintCommand::EndLayer);
        }
        if isolates_clip {
            commands.push(PaintCommand::PopClip);
        }
        if isolates_transform {
            commands.push(PaintCommand::PopTransform);
        }
        Ok(commands)
    }

    /// Convenience for schedules that require no renderer-owned isolation.
    pub fn paint_command(
        &self,
        layout: NativeHostLayout,
        composition: NativeHostComposition,
    ) -> Result<Option<PaintCommand>> {
        let mut commands = self.paint_commands(layout, composition)?;
        if commands.len() > 1 {
            return Err(Error::compile(
                "native_host_scheduler",
                "isolated composition requires the complete paint_commands sequence",
            ));
        }
        Ok(commands.pop())
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NativeHostScheduler;

impl NativeHostScheduler {
    pub fn schedule(
        capabilities: NativeHostCapabilities,
        base_cost: NativeHostCost,
        layout: NativeHostLayout,
    ) -> Result<NativeHostSchedule> {
        let capabilities = capabilities.validate()?;
        let layout = layout.validate()?;
        if !layout.visible || layout.rect.size.is_empty() {
            return Ok(NativeHostSchedule {
                strategy: if capabilities.requires_independent_surface {
                    NativeCompositionStrategy::IndependentSurface
                } else {
                    NativeCompositionStrategy::OffscreenTexture
                },
                visible: false,
                pass_boundary: false,
                z_order: layout.z_order,
                isolation_reasons: Vec::new(),
                cost: NativeHostCost::default(),
                layout,
            });
        }

        let mut isolation_reasons = Vec::new();
        if layout.transform != Transform2D::IDENTITY && !capabilities.supports_transform {
            isolation_reasons.push(NativeIsolationReason::Transform);
        }
        if layout.opacity != 1.0 && !capabilities.supports_alpha {
            isolation_reasons.push(NativeIsolationReason::Alpha);
        }
        if layout.clip.is_some() && !capabilities.supports_clip {
            isolation_reasons.push(NativeIsolationReason::Clip);
        }

        if !isolation_reasons.is_empty() && !capabilities.supports_offscreen {
            return Err(Error::compile(
                "native_host_scheduler",
                format!(
                    "host cannot satisfy {:?}; no offscreen isolation fallback is available",
                    isolation_reasons
                ),
            ));
        }

        let isolated = !isolation_reasons.is_empty();
        let strategy = if capabilities.requires_independent_surface && !isolated {
            NativeCompositionStrategy::IndependentSurface
        } else {
            NativeCompositionStrategy::OffscreenTexture
        };
        let mut cost = base_cost;
        match strategy {
            NativeCompositionStrategy::IndependentSurface => {
                cost.independent_passes = cost.independent_passes.max(1);
                cost.surfaces = cost.surfaces.max(1);
            }
            NativeCompositionStrategy::OffscreenTexture => {
                let width = layout
                    .dpi_scale
                    .logical_to_physical(layout.rect.size.width)
                    .map_err(Error::from)?;
                let height = layout
                    .dpi_scale
                    .logical_to_physical(layout.rect.size.height)
                    .map_err(Error::from)?;
                let bytes = u64::from(width)
                    .saturating_mul(u64::from(height))
                    .saturating_mul(4);
                cost.texture_bytes = cost.texture_bytes.saturating_add(bytes);
                if isolated {
                    cost.independent_passes = cost.independent_passes.saturating_add(1);
                }
            }
        }
        let pass_boundary = strategy == NativeCompositionStrategy::IndependentSurface
            || isolated
            || !capabilities.merges_with_render_batches;
        Ok(NativeHostSchedule {
            strategy,
            visible: true,
            pass_boundary,
            z_order: layout.z_order,
            isolation_reasons,
            cost,
            layout,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MockNativeHostFailure {
    Create,
    Mount,
    Layout,
    Focus,
    Input,
    Compose,
    Unmount,
    Destroy,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MockNativeHostCall {
    Create(NativeHostCreateContext),
    Measure,
    Mount,
    Layout(NativeHostLayout),
    Focus(bool),
    Input(EventKind),
    Compose(NativeCompositionStrategy),
    Unmount,
    Destroy,
}

#[derive(Clone, Debug)]
pub struct MockNativeHostConfig {
    pub capabilities: NativeHostCapabilities,
    pub cost: NativeHostCost,
    pub intrinsic_size: Size,
    pub surface: ResourceId,
    pub opaque: bool,
    pub output_on_input: Option<NativeHostOutput>,
    pub fail_on: Option<MockNativeHostFailure>,
}

impl Default for MockNativeHostConfig {
    fn default() -> Self {
        Self {
            capabilities: NativeHostCapabilities::default(),
            cost: NativeHostCost::default(),
            intrinsic_size: Size::new(320.0, 180.0),
            surface: ResourceId::from_parts(0, 1),
            opaque: true,
            output_on_input: None,
            fail_on: None,
        }
    }
}

/// Deterministic headless factory with a shareable lifecycle call log.
#[derive(Clone, Debug)]
pub struct MockNativeHostFactory {
    config: MockNativeHostConfig,
    calls: Arc<Mutex<Vec<MockNativeHostCall>>>,
}

impl MockNativeHostFactory {
    pub fn new(config: MockNativeHostConfig) -> Self {
        Self {
            config,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn calls(&self) -> Vec<MockNativeHostCall> {
        self.calls.lock().expect("mock host log poisoned").clone()
    }
}

impl Default for MockNativeHostFactory {
    fn default() -> Self {
        Self::new(MockNativeHostConfig::default())
    }
}

impl NativeHostFactory for MockNativeHostFactory {
    fn create(&self, context: NativeHostCreateContext) -> Result<Box<dyn NativeHost>> {
        self.calls
            .lock()
            .expect("mock host log poisoned")
            .push(MockNativeHostCall::Create(context));
        if self.config.fail_on == Some(MockNativeHostFailure::Create) {
            return Err(mock_failure("create"));
        }
        Ok(Box::new(MockNativeHost {
            config: self.config.clone(),
            calls: self.calls.clone(),
        }))
    }
}

struct MockNativeHost {
    config: MockNativeHostConfig,
    calls: Arc<Mutex<Vec<MockNativeHostCall>>>,
}

impl MockNativeHost {
    fn record(&self, call: MockNativeHostCall) {
        self.calls
            .lock()
            .expect("mock host log poisoned")
            .push(call);
    }

    fn check(&self, point: MockNativeHostFailure, operation: &'static str) -> Result<()> {
        if self.config.fail_on == Some(point) {
            Err(mock_failure(operation))
        } else {
            Ok(())
        }
    }
}

impl NativeHost for MockNativeHost {
    fn capabilities(&self) -> NativeHostCapabilities {
        self.config.capabilities
    }

    fn cost(&self) -> NativeHostCost {
        self.config.cost
    }

    fn measure(&self, input: MeasureInput) -> Result<MeasureOutput> {
        self.record(MockNativeHostCall::Measure);
        let mut size = self.config.intrinsic_size;
        if let Some(width) = input.known_dimensions.width {
            size.width = width;
        }
        if let Some(height) = input.known_dimensions.height {
            size.height = height;
        }
        MeasureOutput::new(size).validate()
    }

    fn mount(&mut self) -> Result<Vec<NativeHostOutput>> {
        self.record(MockNativeHostCall::Mount);
        self.check(MockNativeHostFailure::Mount, "mount")?;
        Ok(Vec::new())
    }

    fn update_layout(&mut self, layout: NativeHostLayout) -> Result<Vec<NativeHostOutput>> {
        self.record(MockNativeHostCall::Layout(layout));
        self.check(MockNativeHostFailure::Layout, "update_layout")?;
        Ok(Vec::new())
    }

    fn set_focus(&mut self, focused: bool) -> Result<Vec<NativeHostOutput>> {
        self.record(MockNativeHostCall::Focus(focused));
        self.check(MockNativeHostFailure::Focus, "set_focus")?;
        Ok(Vec::new())
    }

    fn forward_input(&mut self, event: &UiEvent) -> Result<Vec<NativeHostOutput>> {
        self.record(MockNativeHostCall::Input(event.kind()));
        self.check(MockNativeHostFailure::Input, "forward_input")?;
        Ok(self.config.output_on_input.iter().cloned().collect())
    }

    fn compose(&mut self, strategy: NativeCompositionStrategy) -> Result<NativeHostComposition> {
        self.record(MockNativeHostCall::Compose(strategy));
        self.check(MockNativeHostFailure::Compose, "compose")?;
        Ok(NativeHostComposition::new(
            strategy,
            self.config.surface,
            self.config.opaque,
        ))
    }

    fn unmount(&mut self) -> Result<Vec<NativeHostOutput>> {
        self.record(MockNativeHostCall::Unmount);
        self.check(MockNativeHostFailure::Unmount, "unmount")?;
        Ok(Vec::new())
    }

    fn destroy(&mut self) -> Result<()> {
        self.record(MockNativeHostCall::Destroy);
        self.check(MockNativeHostFailure::Destroy, "destroy")
    }
}

fn mock_failure(operation: &'static str) -> Error {
    Error::platform(
        format!("mock_native_host_{operation}"),
        "injected mock host failure",
        true,
    )
}

/// Optional external-surface/WebView contract example. A real platform adapter can
/// replace its opaque resource without introducing WebView dependencies into core.
#[cfg(feature = "webview")]
pub mod webview {
    use super::*;

    #[derive(Clone, Debug)]
    pub struct WebViewHostFactory {
        url: Arc<str>,
        surface: ResourceId,
        size: Size,
    }

    impl WebViewHostFactory {
        pub fn new(url: impl Into<Arc<str>>, surface: ResourceId, size: Size) -> Result<Self> {
            let url = url.into();
            if !(url.starts_with("https://") || url.starts_with("http://")) {
                return Err(Error::invalid_input(
                    Some("webview.url".to_owned()),
                    "URL must use http or https",
                ));
            }
            if !surface.is_well_formed() {
                return Err(Error::invalid_input(
                    Some("webview.surface".to_owned()),
                    "surface must have a non-zero generation",
                ));
            }
            size.validate().map_err(Error::from)?;
            Ok(Self { url, surface, size })
        }

        pub fn url(&self) -> &str {
            &self.url
        }
    }

    impl NativeHostFactory for WebViewHostFactory {
        fn create(&self, _context: NativeHostCreateContext) -> Result<Box<dyn NativeHost>> {
            Ok(Box::new(WebViewHost {
                surface: self.surface,
                size: self.size,
            }))
        }
    }

    struct WebViewHost {
        surface: ResourceId,
        size: Size,
    }

    impl NativeHost for WebViewHost {
        fn capabilities(&self) -> NativeHostCapabilities {
            NativeHostCapabilities {
                requires_independent_surface: true,
                supports_offscreen: false,
                supports_transform: false,
                supports_alpha: false,
                supports_clip: false,
                forwards_pointer: true,
                forwards_keyboard: true,
                forwards_ime: true,
                merges_with_render_batches: false,
            }
        }

        fn cost(&self) -> NativeHostCost {
            NativeHostCost::new(1, 1, 0, 1)
        }

        fn measure(&self, input: MeasureInput) -> Result<MeasureOutput> {
            MeasureOutput::new(Size::new(
                input.known_dimensions.width.unwrap_or(self.size.width),
                input.known_dimensions.height.unwrap_or(self.size.height),
            ))
            .validate()
        }

        fn compose(
            &mut self,
            strategy: NativeCompositionStrategy,
        ) -> Result<NativeHostComposition> {
            if strategy != NativeCompositionStrategy::IndependentSurface {
                return Err(Error::platform(
                    "webview_compose",
                    "example WebView host only exposes an independent surface",
                    true,
                ));
            }
            Ok(NativeHostComposition::new(strategy, self.surface, true))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use crate::event::{PointerEvent, PointerId, PointerKind};
    use crate::layout::{AvailableDimension, AvailableSize, KnownDimensions};

    fn context() -> NativeHostCreateContext {
        NativeHostCreateContext::new(WindowId::from_parts(0, 1), ElementId::from_parts(3, 2))
    }

    fn measure_input() -> MeasureInput {
        MeasureInput {
            known_dimensions: KnownDimensions::default(),
            available_space: AvailableSize {
                width: AvailableDimension::MaxContent,
                height: AvailableDimension::MaxContent,
            },
            style_fingerprint: 0,
            content_generation: 0,
            font_generation: 0,
            scale: DpiScale::ONE,
        }
    }

    #[test]
    fn manager_enforces_lifecycle_and_generation_reuse() {
        let factory = MockNativeHostFactory::default();
        let mut manager = NativeHostManager::new();
        let old = manager.create(&factory, context()).unwrap();
        assert_eq!(
            manager.status(old).unwrap().lifecycle,
            NativeHostLifecycle::Created
        );
        assert_eq!(
            manager.measure(old, measure_input()).unwrap().size.width,
            320.0
        );
        manager.mount(old).unwrap();
        manager
            .update_layout(
                old,
                NativeHostLayout::new(Rect::from_xywh(2.0, 4.0, 80.0, 40.0), DpiScale::ONE)
                    .with_z_order(7),
            )
            .unwrap();
        manager.set_focus(old, true).unwrap();
        manager.unmount(old).unwrap();
        manager.mount(old).unwrap();
        manager.destroy(old).unwrap();
        assert!(!manager.contains(old));

        let current = manager.create(&factory, context()).unwrap();
        assert_eq!(old.slot(), current.slot());
        assert_ne!(old.generation(), current.generation());
        assert!(manager.mount(old).is_err());
        assert!(matches!(
            factory.calls().last(),
            Some(MockNativeHostCall::Create(_))
        ));
    }

    #[test]
    fn input_is_capability_checked_and_output_is_generation_stamped() {
        let output = NativeHostOutput::Command(UiCommand::RequestFrame(context().window));
        let factory = MockNativeHostFactory::new(MockNativeHostConfig {
            output_on_input: Some(output.clone()),
            ..MockNativeHostConfig::default()
        });
        let mut manager = NativeHostManager::new();
        let handle = manager.create(&factory, context()).unwrap();
        manager.mount(handle).unwrap();
        let messages = manager
            .forward_input(
                handle,
                &UiEvent::PointerDown(PointerEvent::new(
                    PointerId::MOUSE,
                    PointerKind::Mouse,
                    Point::new(4.0, 5.0),
                )),
            )
            .unwrap();
        assert_eq!(messages[0].host, handle);
        assert_eq!(messages[0].target, context().element);
        assert_eq!(messages[0].window, context().window);
        assert_eq!(messages[0].output, output);

        let disabled = MockNativeHostFactory::new(MockNativeHostConfig {
            capabilities: NativeHostCapabilities {
                forwards_pointer: false,
                ..NativeHostCapabilities::default()
            },
            ..MockNativeHostConfig::default()
        });
        let blocked = manager.create(&disabled, context()).unwrap();
        manager.mount(blocked).unwrap();
        assert!(
            manager
                .forward_input(
                    blocked,
                    &UiEvent::PointerDown(PointerEvent::new(
                        PointerId::MOUSE,
                        PointerKind::Mouse,
                        Point::ZERO,
                    )),
                )
                .is_err()
        );
    }

    #[test]
    fn scheduler_isolates_or_reports_unsupported_effects_and_costs_them() {
        let layout = NativeHostLayout::new(
            Rect::from_xywh(0.0, 0.0, 100.0, 50.0),
            DpiScale::new(2.0).unwrap(),
        )
        .with_transform(Transform2D::rotation(0.2))
        .with_opacity(0.5)
        .with_clip(Some(Rect::from_xywh(1.0, 1.0, 90.0, 40.0)));
        let isolating = NativeHostCapabilities {
            requires_independent_surface: true,
            supports_offscreen: true,
            supports_transform: false,
            supports_alpha: false,
            supports_clip: false,
            merges_with_render_batches: false,
            ..NativeHostCapabilities::independent_surface()
        };
        let schedule =
            NativeHostScheduler::schedule(isolating, NativeHostCost::new(1, 1, 0, 2), layout)
                .unwrap();
        assert_eq!(
            schedule.strategy,
            NativeCompositionStrategy::OffscreenTexture
        );
        assert_eq!(schedule.isolation_reasons.len(), 3);
        assert!(schedule.pass_boundary);
        assert_eq!(schedule.cost.texture_bytes, 200 * 100 * 4);
        assert_eq!(schedule.cost.independent_passes, 2);
        assert_eq!(schedule.cost.synchronization_points, 2);
        let commands = schedule
            .paint_commands(
                layout,
                NativeHostComposition::new(
                    NativeCompositionStrategy::OffscreenTexture,
                    ResourceId::from_parts(5, 1),
                    true,
                ),
            )
            .unwrap();
        assert!(matches!(
            commands.first(),
            Some(PaintCommand::PushTransform(_))
        ));
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, PaintCommand::PushClip(_)))
        );
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, PaintCommand::BeginLayer(_)))
        );
        assert!(
            commands.iter().any(|command| matches!(
                command,
                PaintCommand::NativeSurface { opaque: false, .. }
            ))
        );
        assert!(matches!(commands.last(), Some(PaintCommand::PopTransform)));
        assert!(
            schedule
                .paint_command(
                    layout,
                    NativeHostComposition::new(
                        NativeCompositionStrategy::OffscreenTexture,
                        ResourceId::from_parts(5, 1),
                        true,
                    ),
                )
                .is_err()
        );

        let unsupported = NativeHostScheduler::schedule(
            NativeHostCapabilities::independent_surface(),
            NativeHostCost::default(),
            layout,
        )
        .unwrap_err();
        assert!(unsupported.to_string().contains("offscreen isolation"));
    }

    #[test]
    fn injected_failure_enters_diagnostic_state_but_can_still_be_destroyed() {
        let factory = MockNativeHostFactory::new(MockNativeHostConfig {
            fail_on: Some(MockNativeHostFailure::Layout),
            ..MockNativeHostConfig::default()
        });
        let mut manager = NativeHostManager::new();
        let handle = manager.create(&factory, context()).unwrap();
        manager.mount(handle).unwrap();
        assert!(
            manager
                .update_layout(
                    handle,
                    NativeHostLayout::new(Rect::from_xywh(0.0, 0.0, 10.0, 10.0), DpiScale::ONE),
                )
                .is_err()
        );
        let status = manager.status(handle).unwrap();
        assert_eq!(status.lifecycle, NativeHostLifecycle::Failed);
        assert_eq!(
            status.last_error.as_ref().unwrap().operation,
            "update_layout"
        );
        manager.destroy(handle).unwrap();
        assert!(!manager.contains(handle));
    }

    #[test]
    fn every_mock_failure_point_has_a_deterministic_fallback() {
        let create = MockNativeHostFactory::new(MockNativeHostConfig {
            fail_on: Some(MockNativeHostFailure::Create),
            ..MockNativeHostConfig::default()
        });
        assert!(NativeHostManager::new().create(&create, context()).is_err());

        for point in [
            MockNativeHostFailure::Mount,
            MockNativeHostFailure::Focus,
            MockNativeHostFailure::Input,
            MockNativeHostFailure::Compose,
            MockNativeHostFailure::Unmount,
        ] {
            let factory = MockNativeHostFactory::new(MockNativeHostConfig {
                fail_on: Some(point),
                ..MockNativeHostConfig::default()
            });
            let mut manager = NativeHostManager::new();
            let handle = manager.create(&factory, context()).unwrap();
            let result = if point == MockNativeHostFailure::Mount {
                manager.mount(handle).map(|_| ())
            } else {
                manager.mount(handle).unwrap();
                match point {
                    MockNativeHostFailure::Focus => manager.set_focus(handle, true).map(|_| ()),
                    MockNativeHostFailure::Input => manager
                        .forward_input(
                            handle,
                            &UiEvent::PointerDown(PointerEvent::new(
                                PointerId::MOUSE,
                                PointerKind::Mouse,
                                Point::ZERO,
                            )),
                        )
                        .map(|_| ()),
                    MockNativeHostFailure::Compose => manager
                        .compose(handle, NativeCompositionStrategy::OffscreenTexture)
                        .map(|_| ()),
                    MockNativeHostFailure::Unmount => manager.unmount(handle).map(|_| ()),
                    _ => unreachable!(),
                }
            };
            assert!(result.is_err());
            assert_eq!(
                manager.status(handle).unwrap().lifecycle,
                NativeHostLifecycle::Failed
            );
            manager.destroy(handle).unwrap();
        }

        let destroy = MockNativeHostFactory::new(MockNativeHostConfig {
            fail_on: Some(MockNativeHostFailure::Destroy),
            ..MockNativeHostConfig::default()
        });
        let mut manager = NativeHostManager::new();
        let handle = manager.create(&destroy, context()).unwrap();
        assert!(manager.destroy(handle).is_err());
        assert!(!manager.contains(handle));
    }
}
