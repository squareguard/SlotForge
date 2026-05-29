#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    DarkOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePalette {
    pub background: &'static str,
    pub background_elevated: &'static str,
    pub panel: &'static str,
    pub panel_border: &'static str,
    pub text_primary: &'static str,
    pub text_secondary: &'static str,
    pub text_muted: &'static str,
    pub accent: &'static str,
    pub accent_soft: &'static str,
    pub success: &'static str,
    pub warning: &'static str,
    pub danger: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeTypography {
    pub family_heading: &'static str,
    pub family_body: &'static str,
    pub size_xs: u8,
    pub size_sm: u8,
    pub size_md: u8,
    pub size_lg: u8,
    pub size_xl: u8,
    pub weight_regular: u16,
    pub weight_semibold: u16,
    pub weight_bold: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeSpacing {
    pub xs: u8,
    pub sm: u8,
    pub md: u8,
    pub lg: u8,
    pub xl: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeIcons {
    pub library: &'static str,
    pub vault: &'static str,
    pub settings: &'static str,
    pub about: &'static str,
    pub warning: &'static str,
    pub success: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppTheme {
    pub mode: ThemeMode,
    pub palette: ThemePalette,
    pub typography: ThemeTypography,
    pub spacing: ThemeSpacing,
    pub icons: ThemeIcons,
}

pub fn dark_hacker_palette() -> ThemePalette {
    ThemePalette {
        background: "#0b0f14",
        background_elevated: "#0f1620",
        panel: "#111821",
        panel_border: "#223244",
        text_primary: "#d7e2f0",
        text_secondary: "#aab8cb",
        text_muted: "#7e8da1",
        accent: "#39d98a",
        accent_soft: "#1c7f55",
        success: "#1dd1a1",
        warning: "#f6b93b",
        danger: "#ff6b6b",
    }
}

pub fn dark_hacker_typography() -> ThemeTypography {
    ThemeTypography {
        family_heading: "Rajdhani",
        family_body: "Inter",
        size_xs: 11,
        size_sm: 13,
        size_md: 15,
        size_lg: 18,
        size_xl: 22,
        weight_regular: 400,
        weight_semibold: 600,
        weight_bold: 700,
    }
}

pub fn dark_hacker_spacing() -> ThemeSpacing {
    ThemeSpacing {
        xs: 4,
        sm: 8,
        md: 12,
        lg: 16,
        xl: 24,
    }
}

pub fn dark_hacker_icons() -> ThemeIcons {
    ThemeIcons {
        library: "icon-library",
        vault: "icon-vault",
        settings: "icon-settings",
        about: "icon-about",
        warning: "icon-warning",
        success: "icon-success",
    }
}

pub fn dark_hacker_theme() -> AppTheme {
    AppTheme {
        mode: ThemeMode::DarkOnly,
        palette: dark_hacker_palette(),
        typography: dark_hacker_typography(),
        spacing: dark_hacker_spacing(),
        icons: dark_hacker_icons(),
    }
}
