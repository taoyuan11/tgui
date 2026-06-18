use super::*;

mod collect;

pub(crate) use collect::portal::{
    build_external_portal_overlay, collect_portal_content_scene, resolve_external_portal_anchor,
};

impl<VM> ResolvedElement<VM> {
    pub(in super::super) fn requires_runtime_lifecycle(&self) -> bool {
        #[cfg(feature = "audio")]
        {
            matches!(&self.kind, ResolvedWidgetKind::Audio { .. })
        }
        #[cfg(not(feature = "audio"))]
        {
            false
        }
    }
}
