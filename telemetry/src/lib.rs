//! The upload allowlist, deliberately independent of miner config and logs.
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;
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
        self.schema_version == SCHEMA_VERSION
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
            && h.supported_cpu_kernels.len() <= 32
            && h.supported_cpu_kernels.iter().all(|s| short(s))
            && (60..=86400).contains(&self.window_secs)
            && self.backends.len() <= 128
            && self.backends.iter().all(|b| {
                matches!(
                    b.settings.backend.as_str(),
                    "cpu" | "nvidia" | "amd" | "metal"
                ) && optional_short(&b.settings.device_model)
                    && optional_short(&b.settings.cpu_kernel)
                    && optional_short(&b.settings.cpu_page_mode)
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
    fn rejects_non_random_identifier_formats() {
        assert!(!valid_id("my-hostname"));
        assert!(!valid_id("0123456789abcdef0123456789abcdeg"));
        assert!(valid_id("0123456789abcdef0123456789abcdef"));
    }
}
