use std::sync::Arc;

use crate::platform::window::Theme as WindowTheme;

use super::mode::ThemeMode;
use super::theme::Theme;

#[derive(Clone, Debug, PartialEq)]
pub struct ThemeSet {
    pub light: Arc<Theme>,
    pub dark: Arc<Theme>,
}

impl Default for ThemeSet {
    fn default() -> Self {
        Self {
            light: Arc::new(Theme::light()),
            dark: Arc::new(Theme::dark()),
        }
    }
}

impl ThemeSet {
    pub fn new(light: Theme, dark: Theme) -> Self {
        Self {
            light: Arc::new(light),
            dark: Arc::new(dark),
        }
    }

    pub fn resolve(&self, mode: ThemeMode, system_theme: Option<WindowTheme>) -> Arc<Theme> {
        match mode {
            ThemeMode::Light => self.light.clone(),
            ThemeMode::Dark => self.dark.clone(),
            ThemeMode::System => self.resolve_window_theme(system_theme),
        }
    }

    pub fn resolve_window_theme(&self, system_theme: Option<WindowTheme>) -> Arc<Theme> {
        match system_theme.unwrap_or(WindowTheme::Dark) {
            WindowTheme::Light => self.light.clone(),
            WindowTheme::Dark => self.dark.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ThemeStore {
    theme_set: ThemeSet,
    mode: ThemeMode,
    system_theme: Option<WindowTheme>,
    current: Arc<Theme>,
    version: u64,
}

impl ThemeStore {
    pub fn new(theme_set: ThemeSet, mode: ThemeMode, system_theme: Option<WindowTheme>) -> Self {
        let current = theme_set.resolve(mode, system_theme);
        Self {
            theme_set,
            mode,
            system_theme,
            current,
            version: 0,
        }
    }

    pub fn current(&self) -> Arc<Theme> {
        self.current.clone()
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn theme_set(&self) -> &ThemeSet {
        &self.theme_set
    }

    pub fn mode(&self) -> ThemeMode {
        self.mode
    }

    pub fn system_theme(&self) -> Option<WindowTheme> {
        self.system_theme
    }

    pub fn set_theme_set(&mut self, theme_set: ThemeSet) -> bool {
        self.theme_set = theme_set;
        self.refresh_current()
    }

    pub fn set_mode(&mut self, mode: ThemeMode) -> bool {
        if self.mode == mode {
            return false;
        }
        self.mode = mode;
        self.refresh_current()
    }

    pub fn set_system_theme(&mut self, system_theme: Option<WindowTheme>) -> bool {
        if self.system_theme == system_theme {
            return false;
        }
        self.system_theme = system_theme;
        self.refresh_current()
    }

    fn refresh_current(&mut self) -> bool {
        let next = self.theme_set.resolve(self.mode, self.system_theme);
        if Arc::ptr_eq(&self.current, &next) {
            return false;
        }
        self.current = next;
        self.version = self.version.wrapping_add(1);
        true
    }
}
