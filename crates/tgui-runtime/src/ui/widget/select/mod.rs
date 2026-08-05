mod option;
mod widget;

pub use option::SelectOption;
pub use widget::Select;

const SELECT_VIRTUAL_LIST_TAG: u64 = 0x5E1E_C7A1_5157_0001;

pub(crate) fn select_virtual_list_id(owner_id: super::WidgetId) -> super::WidgetId {
    super::WidgetId::from_raw(owner_id.raw() ^ SELECT_VIRTUAL_LIST_TAG)
}
