//! The upload allowlist, deliberately independent of miner config and logs.
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 3;
pub const MAX_BODY_BYTES: usize = 32 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub schema_version: u32,
    /// Random per-installation ID; never derived from hardware or a wallet.
    pub installation_id: String,
    pub report_id: String,
    pub miner_version: String,
    pub build_target: String,
    pub build_features: String,
    pub debug_build: bool,
    pub hardware: Hardware,
    pub window_secs: u64,
    pub backends: Vec<BackendSample>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Diagnostics>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCode {
    Warning,
    Error,
    BackendWarning,
    BackendError,
    BackendQuarantined,
    RunFailed,
    RustPanic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticKind {
    General,
    Memory,
    Timeout,
    Driver,
    Compilation,
    Connection,
    RejectedShare,
    Authentication,
    Tuning,
    UnsupportedHardware,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticEvent {
    pub code: DiagnosticCode,
    pub kind: DiagnosticKind,
    /// Repository-relative source location, never a local path or log message.
    pub source: String,
    pub line: u32,
    pub backend: Option<String>,
    pub count: u64,
    pub first_seen_secs: u64,
    pub last_seen_secs: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostics {
    pub build_commit: String,
    pub source_fingerprint: String,
    pub mode: String,
    pub requested_backends: Vec<String>,
    pub requested_cpu_threads: u32,
    pub cpu_page_mode: String,
    pub cpu_affinity: String,
    pub cpu_autotune: bool,
    pub uptime_secs: u64,
    /// periodic, shutdown, or failure; short final reports are not performance windows.
    pub reason: String,
    /// An unfinished previous run, which may be a power loss or forced stop, not proof of a crash.
    pub previous_run_unclean: bool,
    pub dropped_events: u64,
    pub dropped_backend_samples: u32,
    pub events: Vec<DiagnosticEvent>,
}

pub fn valid_source(source: &str) -> bool {
    source == "unknown"
        || (source.starts_with("src/")
            && source.ends_with(".rs")
            && source.len() <= 120
            && !source.contains("..")
            && source
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_/.".contains(&b)))
}

fn backend_name(value: &str) -> bool {
    matches!(value, "cpu" | "nvidia" | "amd" | "metal")
}

impl Diagnostics {
    fn validate(&self) -> bool {
        let fingerprint = |s: &str| {
            s == "unknown"
                || (!s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        };
        fingerprint(&self.build_commit)
            && fingerprint(&self.source_fingerprint)
            && matches!(self.mode.as_str(), "pool" | "daemon")
            && self.requested_backends.len() <= 4
            && self.requested_backends.iter().all(|b| backend_name(b))
            && self.requested_cpu_threads <= 65536
            && matches!(
                self.cpu_page_mode.as_str(),
                "auto" | "regular" | "large" | "large-1g"
            )
            && matches!(
                self.cpu_affinity.as_str(),
                "auto" | "os" | "off" | "pcore-only"
            )
            && matches!(self.reason.as_str(), "periodic" | "shutdown" | "failure")
            && self.events.len() <= 32
            && self.uptime_secs <= 315_360_000
            && self.dropped_events <= 1_000_000_000_000
            && self.dropped_backend_samples <= 128
            && self.events.iter().all(|e| {
                valid_source(&e.source)
                    && e.line <= 1_000_000
                    && e.backend.as_deref().is_none_or(backend_name)
                    && e.count > 0
                    && e.count <= 1_000_000_000_000
                    && e.first_seen_secs <= e.last_seen_secs
                    && e.last_seen_secs <= self.uptime_secs
            })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hardware {
    pub os: String,
    pub os_version: Option<String>,
    pub runtime_environment: String,
    pub cpu_model: String,
    pub cpu_vendor: Option<String>,
    pub cpu_family: Option<u32>,
    pub cpu_model_id: Option<u32>,
    pub logical_cpus: u32,
    pub physical_cores: Option<u32>,
    pub memory_gib: u32,
    pub supported_cpu_kernels: Vec<String>,
    /// Report-time resource snapshot; absent in schema 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<Resources>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    pub allowed_logical_cpus: Option<u32>,
    /// OS estimate including CPU quotas where supported, not a measured duty cycle.
    pub available_parallelism: Option<u32>,
    pub effective_memory_gib: Option<u32>,
    pub available_memory_gib: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackendSettings {
    pub backend: String,
    pub device_model: Option<String>,
    pub memory_mib: Option<u64>,
    pub lanes: u32,
    pub cpu_kernel: Option<String>,
    pub cpu_page_mode: Option<String>,
    pub gpu_register_cap: Option<u32>,
    pub gpu_launch_depth: Option<u32>,
    /// Allocator-observed MiB: explicit large, 1 GiB, THP, regular, heap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_backing_mib: Option<[u64; 5]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_affinity: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackendSample {
    /// Session-local index, not a device serial or persistent identifier.
    pub instance: u64,
    pub settings: BackendSettings,
    pub hashes: u64,
    pub observed_secs: f64,
    /// Omit comparisons when tuning changed inside the reporting window.
    pub settings_changed: bool,
}

fn short(value: &str) -> bool {
    !value.is_empty() && value.len() <= 160 && !value.chars().any(char::is_control)
}

fn optional_short(value: &Option<String>) -> bool {
    value.as_deref().is_none_or(short)
}

pub fn valid_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

impl Report {
    pub fn validate(&self) -> bool {
        let h = &self.hardware;
        (1..=SCHEMA_VERSION).contains(&self.schema_version)
            && valid_id(&self.installation_id)
            && valid_id(&self.report_id)
            && [
                &self.miner_version,
                &self.build_target,
                &self.build_features,
                &h.os,
                &h.runtime_environment,
                &h.cpu_model,
            ]
            .into_iter()
            .all(|s| short(s))
            && optional_short(&h.os_version)
            && optional_short(&h.cpu_vendor)
            && h.logical_cpus <= 65536
            && h.physical_cores.is_none_or(|v| v <= 65536)
            && h.memory_gib <= 1048576
            && h.resources.as_ref().is_none_or(|r| {
                r.allowed_logical_cpus.is_none_or(|v| v <= 65536)
                    && r.available_parallelism.is_none_or(|v| v <= 65536)
                    && r.effective_memory_gib.is_none_or(|v| v <= 1048576)
                    && r.available_memory_gib.is_none_or(|v| v <= 1048576)
            })
            && (self.schema_version >= 2
                || (h.resources.is_none()
                    && self.backends.iter().all(|b| {
                        b.settings.cpu_backing_mib.is_none() && b.settings.cpu_affinity.is_none()
                    })))
            && h.supported_cpu_kernels.len() <= 32
            && h.supported_cpu_kernels.iter().all(|s| short(s))
            && (1..=86400).contains(&self.window_secs)
            && (self.window_secs >= 60
                || self
                    .diagnostics
                    .as_ref()
                    .is_some_and(|d| d.reason != "periodic"))
            && self
                .diagnostics
                .as_ref()
                .is_none_or(|d| self.schema_version >= 3 && d.validate())
            && self.backends.len() <= 128
            && self.backends.iter().all(|b| {
                matches!(
                    b.settings.backend.as_str(),
                    "cpu" | "nvidia" | "amd" | "metal"
                ) && optional_short(&b.settings.device_model)
                    && optional_short(&b.settings.cpu_kernel)
                    && optional_short(&b.settings.cpu_page_mode)
                    && b.settings
                        .cpu_affinity
                        .as_deref()
                        .is_none_or(|v| matches!(v, "auto" | "os" | "off" | "pcore-only"))
                    && b.settings
                        .cpu_backing_mib
                        .is_none_or(|v| v.iter().all(|&mib| mib <= 1073741824))
                    && (b.settings.backend == "cpu"
                        || (b.settings.cpu_backing_mib.is_none()
                            && b.settings.cpu_affinity.is_none()))
                    && b.settings.lanes <= 65536
                    && b.settings.memory_mib.is_none_or(|v| v <= 1073741824)
                    && b.observed_secs.is_finite()
                    && b.observed_secs > 0.0
                    && b.observed_secs <= self.window_secs as f64 + 1.0
                    && b.hashes <= 1_000_000_000_000_000
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_are_allowlisted_bounded_and_versioned() {
        let value: serde_json::Value =
            serde_json::from_str(include_str!("../diagnostics-fixture.json")).unwrap();
        let report: Report = serde_json::from_value(value.clone()).unwrap();
        assert!(report.validate());
        for field in ["message", "stacktrace", "wallet", "command_line"] {
            let mut bad = value.clone();
            bad["diagnostics"]["events"][0][field] = "secret".into();
            assert!(serde_json::from_value::<Report>(bad).is_err());
        }
        for (field, secret) in [
            ("code", "private-error-text"),
            ("kind", "private-error-text"),
        ] {
            let mut bad = value.clone();
            bad["diagnostics"]["events"][0][field] = secret.into();
            assert!(serde_json::from_value::<Report>(bad).is_err());
        }
        for path in [
            "/home/alice/src/main.rs",
            "src/../../private.rs",
            "C:\\Users\\Alice\\main.rs",
        ] {
            let mut bad = report.clone();
            bad.diagnostics.as_mut().unwrap().events[0].source = path.into();
            assert!(!bad.validate());
        }
        let mut bad = report.clone();
        bad.schema_version = 2;
        assert!(!bad.validate());
        let mut bad = report.clone();
        let d = bad.diagnostics.as_mut().unwrap();
        d.events = vec![d.events[0].clone(); 33];
        assert!(!bad.validate());
        let mut final_report = report;
        final_report.backends.clear();
        final_report.window_secs = 1;
        assert!(!final_report.validate());
        final_report.diagnostics.as_mut().unwrap().reason = "failure".into();
        assert!(final_report.validate());
    }

    #[test]
    fn strict_schema_and_numeric_limits_reject_unintended_data() {
        let value: serde_json::Value =
            serde_json::from_str(include_str!("../fixture.json")).unwrap();
        let report: Report = serde_json::from_value(value.clone()).unwrap();
        assert!(report.validate());
        for path in [
            "wallet_address",
            "hostname",
            "logs",
            "worker_name",
            "api_token",
        ] {
            let mut bad = value.clone();
            bad[path] = "private".into();
            assert!(serde_json::from_value::<Report>(bad).is_err());
        }
        let mut bad = value.clone();
        bad["hardware"]["hostname"] = "private".into();
        assert!(serde_json::from_value::<Report>(bad).is_err());
        let mut report: Report = serde_json::from_value(value).unwrap();
        report.backends[0].observed_secs = f64::NAN;
        assert!(!report.validate());
        report.backends[0].observed_secs = 99999.0;
        assert!(!report.validate());
        report.backends[0].observed_secs = 3600.0;
        report.hardware.cpu_model = "x".repeat(161);
        assert!(!report.validate());
    }

    #[test]
    fn accepts_legacy_and_extended_reports_without_confusing_unknown_with_zero() {
        let value: serde_json::Value =
            serde_json::from_str(include_str!("../fixture.json")).unwrap();
        let mut legacy = value.clone();
        legacy["schema_version"] = 1.into();
        legacy["hardware"]
            .as_object_mut()
            .unwrap()
            .remove("resources");
        for backend in legacy["backends"].as_array_mut().unwrap() {
            backend["settings"]
                .as_object_mut()
                .unwrap()
                .remove("cpu_backing_mib");
            backend["settings"]
                .as_object_mut()
                .unwrap()
                .remove("cpu_affinity");
        }
        let old: Report = serde_json::from_value(legacy).unwrap();
        assert!(old.validate());
        assert!(old.hardware.resources.is_none());
        assert!(old.backends[0].settings.cpu_backing_mib.is_none());
        let mut extended: Report = serde_json::from_value(value).unwrap();
        assert!(extended.validate());
        extended.schema_version = 1;
        assert!(!extended.validate(), "new fields must advertise schema 2");
        extended.schema_version = SCHEMA_VERSION + 1;
        assert!(!extended.validate());
        extended.schema_version = 2;
        extended.backends[0].settings.cpu_backing_mib = Some([u64::MAX; 5]);
        assert!(!extended.validate());
    }

    #[test]
    fn rejects_non_random_identifier_formats() {
        assert!(!valid_id("my-hostname"));
        assert!(!valid_id("0123456789abcdef0123456789abcdeg"));
        assert!(valid_id("0123456789abcdef0123456789abcdef"));
    }
}
