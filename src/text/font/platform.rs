use cosmic_text::fontdb::ID;

#[cfg(any(target_os = "android", target_env = "ohos"))]
pub(super) fn load_mobile_system_fonts(database: &mut cosmic_text::fontdb::Database) {
    for path in mobile_font_dirs() {
        let path = std::path::Path::new(path);
        if path.exists() {
            database.load_fonts_dir(path);
        }
    }

    let sans_family = first_matching_family(database, mobile_sans_candidates())
        .or_else(|| first_loaded_family(database));

    let serif_family =
        first_matching_family(database, mobile_serif_candidates()).or_else(|| sans_family.clone());

    let monospace_family = first_matching_family(database, mobile_monospace_candidates())
        .or_else(|| sans_family.clone());

    if let Some(family) = sans_family {
        database.set_sans_serif_family(family.clone());
        database.set_cursive_family(family.clone());
        database.set_fantasy_family(family);
    }
    if let Some(family) = serif_family {
        database.set_serif_family(family);
    }
    if let Some(family) = monospace_family {
        database.set_monospace_family(family);
    }
}

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

#[cfg(target_os = "android")]
fn mobile_font_dirs() -> &'static [&'static str] {
    &[
        "/system/fonts",
        "/system_ext/fonts",
        "/product/fonts",
        "/vendor/fonts",
    ]
}

#[cfg(target_env = "ohos")]
fn mobile_font_dirs() -> &'static [&'static str] {
    &[
        "/system/fonts",
        "/system/etc/fonts",
        "/system/fonts/visibility",
        "/data/service/el1/public/font",
    ]
}

#[cfg(target_os = "android")]
fn mobile_sans_candidates() -> &'static [&'static str] {
    &[
        "Roboto",
        "Roboto Static",
        "Roboto Flex",
        "Droid Sans",
        "Noto Sans CJK SC",
        "Noto Sans CJK TC",
        "Noto Sans CJK JP",
        "Noto Sans CJK KR",
        "Noto Sans",
    ]
}

#[cfg(target_env = "ohos")]
fn mobile_sans_candidates() -> &'static [&'static str] {
    &[
        "HarmonyOS Sans SC",
        "HarmonyOS Sans",
        "Noto Sans CJK SC",
        "Noto Sans SC",
        "Noto Sans",
    ]
}

#[cfg(target_os = "android")]
fn mobile_serif_candidates() -> &'static [&'static str] {
    &[
        "Noto Serif",
        "Noto Serif CJK SC",
        "Noto Serif CJK TC",
        "Noto Serif CJK JP",
        "Noto Serif CJK KR",
    ]
}

#[cfg(target_env = "ohos")]
fn mobile_serif_candidates() -> &'static [&'static str] {
    &[
        "Noto Serif CJK SC",
        "Noto Serif SC",
        "Noto Serif",
        "HarmonyOS Sans SC",
    ]
}

#[cfg(target_os = "android")]
fn mobile_monospace_candidates() -> &'static [&'static str] {
    &[
        "Droid Sans Mono",
        "Cutive Mono",
        "Roboto Mono",
        "Noto Sans Mono",
    ]
}

#[cfg(target_env = "ohos")]
fn mobile_monospace_candidates() -> &'static [&'static str] {
    &[
        "HarmonyOS Sans Mono",
        "Roboto Mono",
        "Noto Sans Mono",
        "HarmonyOS Sans SC",
    ]
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

#[cfg(any(target_os = "android", target_env = "ohos"))]
fn first_loaded_family(database: &cosmic_text::fontdb::Database) -> Option<String> {
    database
        .faces()
        .find_map(|face| face.families.first().map(|(family, _)| family.clone()))
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

#[cfg(all(target_os = "linux", not(target_env = "ohos")))]
pub(super) fn desktop_cjk_sans_candidates() -> Option<&'static [&'static str]> {
    Some(&[
        "Noto Sans CJK SC",
        "Noto Sans SC",
        "Source Han Sans SC",
        "WenQuanYi Micro Hei",
        "Droid Sans Fallback",
    ])
}

#[cfg(any(target_os = "android", target_env = "ohos"))]
pub(super) fn desktop_cjk_sans_candidates() -> Option<&'static [&'static str]> {
    None
}

pub(super) fn face_family_name(database: &cosmic_text::fontdb::Database, id: ID) -> Option<String> {
    database
        .face(id)
        .and_then(|face| face.families.first().map(|(family, _)| family.clone()))
}
