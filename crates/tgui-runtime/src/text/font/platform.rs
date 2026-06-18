use cosmic_text::fontdb::ID;

pub(super) fn contains_cjk(text: &str) -> bool {
    text.chars().any(is_cjk_character)
}

pub(super) fn contains_non_cjk_alphanumeric(text: &str) -> bool {
    text.chars()
        .any(|ch| !is_cjk_character(ch) && ch.is_alphanumeric())
}

fn is_cjk_character(ch: char) -> bool {
    matches!(
        ch as u32,
        0x2E80..=0x2EFF
            | 0x2F00..=0x2FDF
            | 0x3000..=0x303F
            | 0x31C0..=0x31EF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0x2CEB0..=0x2EBEF
            | 0x2F800..=0x2FA1F
    )
}

pub(super) fn first_matching_family(
    database: &cosmic_text::fontdb::Database,
    candidates: &[&str],
) -> Option<String> {
    candidates.iter().find_map(|candidate| {
        database.faces().find_map(|face| {
            face.families
                .iter()
                .find(|(family, _)| family.eq_ignore_ascii_case(candidate))
                .map(|(family, _)| family.clone())
        })
    })
}

#[cfg(target_os = "windows")]
pub(super) fn desktop_cjk_sans_candidates() -> Option<&'static [&'static str]> {
    Some(&[
        "Noto Sans SC",
        "DengXian",
        "Microsoft YaHei",
        "Microsoft YaHei UI",
        "Microsoft JhengHei UI",
        "Microsoft JhengHei",
        "SimHei",
        "Yu Gothic UI",
        "Yu Gothic",
        "Malgun Gothic",
        "SimSun",
    ])
}

#[cfg(target_os = "macos")]
pub(super) fn desktop_cjk_sans_candidates() -> Option<&'static [&'static str]> {
    Some(&[
        "PingFang SC",
        "Hiragino Sans GB",
        "Heiti SC",
        "Apple SD Gothic Neo",
    ])
}

#[cfg(target_os = "linux")]
pub(super) fn desktop_cjk_sans_candidates() -> Option<&'static [&'static str]> {
    Some(&[
        "Noto Sans CJK SC",
        "Noto Sans SC",
        "Source Han Sans SC",
        "WenQuanYi Micro Hei",
        "Droid Sans Fallback",
    ])
}

pub(super) fn face_family_name(database: &cosmic_text::fontdb::Database, id: ID) -> Option<String> {
    database
        .face(id)
        .and_then(|face| face.families.first().map(|(family, _)| family.clone()))
}
