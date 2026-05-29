#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppSection {
    Library,
    Vault,
    Settings,
    About,
}

pub fn ordered_sections() -> [AppSection; 4] {
    [
        AppSection::Library,
        AppSection::Vault,
        AppSection::Settings,
        AppSection::About,
    ]
}

pub fn section_label(section: AppSection) -> &'static str {
    match section {
        AppSection::Library => "Library",
        AppSection::Vault => "Vault",
        AppSection::Settings => "Settings",
        AppSection::About => "About",
    }
}
