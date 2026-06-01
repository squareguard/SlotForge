use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::platform::fs::ensure_directory;
use crate::services::config_service;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricOperation {
    Backup,
    Delete,
    Swap,
    Restore,
}

impl MetricOperation {
    fn as_key(self) -> &'static str {
        match self {
            MetricOperation::Backup => "backup",
            MetricOperation::Delete => "delete",
            MetricOperation::Swap => "swap",
            MetricOperation::Restore => "restore",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct OperationCounters {
    attempts: u64,
    successes: u64,
    failures: u64,
    user_errors: u64,
    recovered_failures: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetricsRegistry {
    operations: BTreeMap<String, OperationCounters>,
    updated_at: DateTime<Utc>,
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self {
            operations: BTreeMap::new(),
            updated_at: Utc::now(),
        }
    }
}

/// MVP targets used to judge release readiness from persisted operation counters.
#[derive(Debug, Clone, PartialEq)]
pub struct MvpSuccessCriteria {
    pub min_operation_success_rate: f64,
    pub max_swap_failure_rate: f64,
    pub max_restore_failure_rate: f64,
    pub min_user_error_recoverability_rate: f64,
}

impl Default for MvpSuccessCriteria {
    fn default() -> Self {
        Self {
            min_operation_success_rate: 0.95,
            max_swap_failure_rate: 0.05,
            max_restore_failure_rate: 0.05,
            min_user_error_recoverability_rate: 0.90,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MvpCriteriaEvaluation {
    pub meets_criteria: bool,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetricsSnapshot {
    pub operation_success_rate: f64,
    pub swap_failure_rate: f64,
    pub restore_failure_rate: f64,
    pub user_error_recoverability_rate: f64,
    pub total_attempts: u64,
    pub total_failures: u64,
    pub total_recovered_failures: u64,
}

pub fn record_operation(
    operation: MetricOperation,
    success: bool,
    user_error: bool,
    recovered_after_failure: bool,
) -> Result<()> {
    let mut registry = load_registry()?;
    let counters = registry
        .operations
        .entry(operation.as_key().to_string())
        .or_default();

    counters.attempts += 1;
    if success {
        counters.successes += 1;
    } else {
        counters.failures += 1;
    }
    if user_error {
        counters.user_errors += 1;
    }
    if recovered_after_failure {
        counters.recovered_failures += 1;
    }
    registry.updated_at = Utc::now();
    save_registry(&registry)
}

/// Records metrics without failing the caller; logs persistence errors.
pub fn record_operation_best_effort(
    operation: MetricOperation,
    success: bool,
    user_error: bool,
    recovered_after_failure: bool,
) {
    if let Err(err) = record_operation(
        operation,
        success,
        user_error,
        recovered_after_failure,
    ) {
        warn!("failed to record metrics for {operation:?}: {err:#}");
    }
}

pub fn read_snapshot() -> Result<MetricsSnapshot> {
    let registry = load_registry()?;
    let total_attempts = registry.operations.values().map(|c| c.attempts).sum::<u64>();
    let total_successes = registry.operations.values().map(|c| c.successes).sum::<u64>();
    let total_failures = registry.operations.values().map(|c| c.failures).sum::<u64>();
    let total_user_errors = registry.operations.values().map(|c| c.user_errors).sum::<u64>();
    let total_recovered_failures = registry
        .operations
        .values()
        .map(|c| c.recovered_failures)
        .sum::<u64>();

    let swap_failure_rate = operation_failure_rate(&registry, MetricOperation::Swap);
    let restore_failure_rate = operation_failure_rate(&registry, MetricOperation::Restore);

    Ok(MetricsSnapshot {
        operation_success_rate: rate(total_successes, total_attempts),
        swap_failure_rate,
        restore_failure_rate,
        user_error_recoverability_rate: rate(total_recovered_failures, total_user_errors),
        total_attempts,
        total_failures,
        total_recovered_failures,
    })
}

pub fn evaluate_mvp_criteria(
    snapshot: &MetricsSnapshot,
    criteria: &MvpSuccessCriteria,
) -> MvpCriteriaEvaluation {
    let mut failures = Vec::new();

    if snapshot.total_attempts > 0
        && snapshot.operation_success_rate < criteria.min_operation_success_rate
    {
        failures.push(format!(
            "operation success rate {:.2}% is below MVP target {:.2}%",
            snapshot.operation_success_rate * 100.0,
            criteria.min_operation_success_rate * 100.0
        ));
    }
    if snapshot.swap_failure_rate > criteria.max_swap_failure_rate {
        failures.push(format!(
            "swap failure rate {:.2}% exceeds MVP target {:.2}%",
            snapshot.swap_failure_rate * 100.0,
            criteria.max_swap_failure_rate * 100.0
        ));
    }
    if snapshot.restore_failure_rate > criteria.max_restore_failure_rate {
        failures.push(format!(
            "restore failure rate {:.2}% exceeds MVP target {:.2}%",
            snapshot.restore_failure_rate * 100.0,
            criteria.max_restore_failure_rate * 100.0
        ));
    }
    if snapshot.total_failures > 0
        && snapshot.user_error_recoverability_rate < criteria.min_user_error_recoverability_rate
    {
        failures.push(format!(
            "user-error recoverability {:.2}% is below MVP target {:.2}%",
            snapshot.user_error_recoverability_rate * 100.0,
            criteria.min_user_error_recoverability_rate * 100.0
        ));
    }

    MvpCriteriaEvaluation {
        meets_criteria: failures.is_empty(),
        failures,
    }
}

fn operation_failure_rate(registry: &MetricsRegistry, operation: MetricOperation) -> f64 {
    registry
        .operations
        .get(operation.as_key())
        .map(|counters| rate(counters.failures, counters.attempts))
        .unwrap_or(0.0)
}

fn load_registry() -> Result<MetricsRegistry> {
    let path = metrics_registry_path();
    if !path.exists() {
        return Ok(MetricsRegistry::default());
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read metrics registry at {}", path.display()))?;
    let registry = serde_json::from_str::<MetricsRegistry>(&raw)
        .with_context(|| format!("failed to parse metrics registry at {}", path.display()))?;
    Ok(registry)
}

fn save_registry(registry: &MetricsRegistry) -> Result<()> {
    let path = metrics_registry_path();
    if let Some(parent) = path.parent() {
        ensure_directory(parent)?;
    }
    let body =
        serde_json::to_string_pretty(registry).context("failed to serialize metrics registry")?;
    fs::write(&path, body)
        .with_context(|| format!("failed to write metrics registry at {}", path.display()))?;
    Ok(())
}

fn metrics_registry_path() -> PathBuf {
    let config_path = config_service::config_path();
    if let Some(parent) = config_path.parent() {
        return parent.join("metrics.json");
    }
    PathBuf::from("slotforge-metrics.json")
}

fn rate(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        return 0.0;
    }
    numerator as f64 / denominator as f64
}

pub fn is_likely_user_error(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("confirmation required")
        || normalized.contains("cancelled by user")
        || normalized.contains("swap blocked")
        || normalized.contains("delete blocked")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        evaluate_mvp_criteria, is_likely_user_error, read_snapshot, record_operation,
        MetricOperation, MvpSuccessCriteria,
    };

    #[test]
    fn computes_expected_success_and_failure_rates() {
        let config_path = unique_temp_file("metrics-config");
        // SAFETY: scoped, test-only env var override with cleanup at the end.
        unsafe { std::env::set_var("SLOTFORGE_CONFIG_PATH", config_path.to_string_lossy().to_string()) };

        record_operation(MetricOperation::Backup, true, false, false).expect("record backup");
        record_operation(MetricOperation::Swap, false, true, true).expect("record failed swap");
        record_operation(MetricOperation::Swap, true, false, false).expect("record successful swap");

        let snapshot = read_snapshot().expect("read snapshot");
        assert!((snapshot.operation_success_rate - (2.0 / 3.0)).abs() < f64::EPSILON);
        assert!((snapshot.swap_failure_rate - 0.5).abs() < f64::EPSILON);
        assert!((snapshot.user_error_recoverability_rate - 1.0).abs() < f64::EPSILON);

        let metrics_path = config_path
            .parent()
            .expect("temp config path should have a parent directory")
            .join("metrics.json");
        let _ = fs::remove_file(metrics_path);
        let _ = fs::remove_file(config_path);
        unsafe { std::env::remove_var("SLOTFORGE_CONFIG_PATH") };
    }

    #[test]
    fn evaluates_mvp_criteria_against_snapshot() {
        use super::MetricsSnapshot;

        let snapshot = MetricsSnapshot {
            operation_success_rate: 0.98,
            swap_failure_rate: 0.02,
            restore_failure_rate: 0.0,
            user_error_recoverability_rate: 0.95,
            total_attempts: 100,
            total_failures: 2,
            total_recovered_failures: 1,
        };
        let evaluation = evaluate_mvp_criteria(&snapshot, &MvpSuccessCriteria::default());
        assert!(evaluation.meets_criteria);
        assert!(evaluation.failures.is_empty());
    }

    #[test]
    fn identifies_user_safety_and_confirmation_errors() {
        assert!(is_likely_user_error("swap blocked: confirmation required before replacing active save"));
        assert!(is_likely_user_error("swap cancelled by user choice"));
        assert!(!is_likely_user_error("failed to copy selected save into active location"));
    }

    fn unique_temp_file(prefix: &str) -> std::path::PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("slotforge_{prefix}_{ts}.json"))
    }
}
