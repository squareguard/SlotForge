mod startup;

use crate::ui::navigation::{self, AppSection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppShellState {
    pub active_section: AppSection,
    pub sections: Vec<AppSection>,
}

impl AppShellState {
    pub fn new() -> Self {
        Self {
            active_section: AppSection::Library,
            sections: navigation::ordered_sections().to_vec(),
        }
    }

    pub fn navigate_to(&mut self, section: AppSection) {
        self.active_section = section;
    }

    pub fn active_section_label(&self) -> &'static str {
        navigation::section_label(self.active_section)
    }
}

pub fn run() -> anyhow::Result<()> {
    crate::services::audit_service::init_logging();

    if let Err(err) = crate::services::config_service::ensure_initialized() {
        let _ = crate::services::audit_service::record_event(
            &crate::services::audit_service::AuditEvent {
                timestamp: chrono::Utc::now(),
                action: crate::services::audit_service::AuditAction::AppStartup,
                outcome: crate::services::audit_service::AuditOutcome::Failure,
                message: format!("config initialization failed: {err}"),
                game_id: None,
                source_path: None,
                destination_path: None,
            },
        );
        return Err(err);
    }

    let _ = crate::services::audit_service::record_event(
        &crate::services::audit_service::AuditEvent {
            timestamp: chrono::Utc::now(),
            action: crate::services::audit_service::AuditAction::AppStartup,
            outcome: crate::services::audit_service::AuditOutcome::Success,
            message: "slotforge bootstrap completed".to_string(),
            game_id: None,
            source_path: None,
            destination_path: None,
        },
    );

    let _theme = crate::ui::theme::dark_hacker_theme();
    let mut shell = AppShellState::new();
    startup::run_startup_report(&mut shell)?;

    if std::env::var("SLOTFORGE_SELF_TEST").as_deref() == Ok("1") {
        startup::run_self_test()?;
        println!("Self-test completed successfully.");
    }

    Ok(())
}
