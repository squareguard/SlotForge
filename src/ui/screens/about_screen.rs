pub fn title() -> &'static str {
    "About"
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AboutInfo {
    pub app_name: &'static str,
    pub purpose: &'static str,
    pub reliability_guarantees: Vec<&'static str>,
    pub version: &'static str,
    pub build_target: &'static str,
    pub build_profile: &'static str,
}

pub fn about_info() -> AboutInfo {
    AboutInfo {
        app_name: "SlotForge",
        purpose: "Securely manage, back up, and hot-swap PC game save files across platforms.",
        reliability_guarantees: vec![
            "Conflict-aware save operations with explicit safety defaults",
            "Rollback-capable swap transactions to prevent data loss",
            "Metadata + hash verification for vault and swap workflows",
            "Persistent audit logging for high-risk file operations",
            "MVP instrumentation tracks operation success, swap/restore failure, and user-error recoverability",
        ],
        version: env!("CARGO_PKG_VERSION"),
        build_target: std::env::consts::OS,
        build_profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
    }
}
