use super::*;

mod collect;

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
