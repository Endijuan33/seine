//! Optional, best-effort hardware reporting. No wallet/config/log serialization.
use anyhow::{bail, Context, Result};
use crossbeam_channel::{bounded, Sender};
use seine_telemetry::{BackendSample, BackendSettings, Hardware, Report, SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const INTERVAL: Duration = Duration::from_secs(3600);
const SETTINGS_FILE: &str = "seine.telemetry.json";

#[derive(Clone, Debug)]
pub(crate) struct Options {
    pub endpoint: String,
    pub installation_id: String,
    pub state_path: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    enabled: bool,
    endpoint: Option<String>,
    installation_id: Option<String>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: None,
            installation_id: None,
        }
    }
}

fn random_id() -> Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|e| anyhow::anyhow!("random ID: {e}"))?;
    Ok(hex::encode(bytes))
}

pub(crate) fn resolve(
    data_dir: &Path,
    choice: Option<bool>,
    endpoint: Option<&str>,
) -> Result<Option<Options>> {
    resolve_with_default(
        data_dir,
        choice,
        endpoint,
        option_env!("SEINE_TELEMETRY_URL"),
    )
}

fn resolve_with_default(
    data_dir: &Path,
    choice: Option<bool>,
    endpoint: Option<&str>,
    default_endpoint: Option<&str>,
) -> Result<Option<Options>> {
    let path = data_dir.join(SETTINGS_FILE);
    let mut preferences: Preferences = match fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(c) => c,
            Err(_) if choice == Some(false) => Preferences::default(),
            Err(e) => return Err(e).context("invalid telemetry settings"),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => Preferences::default(),
        Err(e) => return Err(e.into()),
    };
    let mut changed = false;
    let endpoint = endpoint.or_else(|| {
        if preferences.endpoint.is_none() {
            default_endpoint.filter(|v| !v.is_empty())
        } else {
            None
        }
    });
    if let Some(enabled) = choice {
        preferences.enabled = enabled;
        changed = true;
    }
    if let Some(endpoint) = endpoint {
        validate_endpoint(endpoint)?;
        if preferences.endpoint.as_deref() != Some(endpoint) {
            preferences.installation_id = None;
        }
        preferences.endpoint = Some(endpoint.to_owned());
        changed = true;
    }
    if choice == Some(true) && preferences.endpoint.is_none() {
        bail!("--telemetry on requires --telemetry-url https://your-collector/v1/reports on first use");
    }
    if preferences.enabled
        && preferences.endpoint.is_some()
        && preferences
            .installation_id
            .as_deref()
            .is_none_or(|id| !seine_telemetry::valid_id(id))
    {
        preferences.installation_id = Some(random_id()?);
        changed = true;
    }
    if changed {
        fs::create_dir_all(data_dir)?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(&preferences)?)?;
        fs::rename(temporary, &path)?;
    }
    if !preferences.enabled || preferences.endpoint.is_none() {
        return Ok(None);
    }
    let endpoint = preferences.endpoint.unwrap();
    validate_endpoint(&endpoint)?;
    Ok(Some(Options {
        endpoint,
        installation_id: preferences.installation_id.unwrap(),
        state_path: path,
    }))
}

fn validate_endpoint(value: &str) -> Result<()> {
    let url = reqwest::Url::parse(value)?;
    let loopback = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    if (url.scheme() != "https" && !(loopback && url.scheme() == "http"))
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        bail!("telemetry URL must use HTTPS without credentials, query or fragment (HTTP permitted only on loopback)");
    }
    Ok(())
}

fn still_enabled(options: &Options) -> bool {
    fs::read(&options.state_path)
        .ok()
        .and_then(|v| serde_json::from_slice::<Preferences>(&v).ok())
        .is_some_and(|c| {
            c.enabled
                && c.endpoint.as_deref() == Some(&options.endpoint)
                && c.installation_id.as_deref() == Some(&options.installation_id)
        })
}

#[derive(Clone)]
pub(crate) struct Recorder(Arc<Mutex<BTreeMap<u64, Accumulated>>>, Arc<AtomicBool>);

struct Accumulated {
    settings: BackendSettings,
    hashes: u64,
    first: Instant,
    last: Instant,
    changed: bool,
}

impl Recorder {
    pub(crate) fn record(&self, instance: u64, settings: BackendSettings, hashes: u64) {
        // The miner never waits for a report to finish; contention loses telemetry only.
        let Ok(mut samples) = self.0.try_lock() else {
            self.1.store(true, Ordering::Relaxed);
            return;
        };
        if samples.len() >= 128 && !samples.contains_key(&instance) {
            self.1.store(true, Ordering::Relaxed);
            return;
        }
        let now = Instant::now();
        let first_sample = !samples.contains_key(&instance);
        let sample = samples.entry(instance).or_insert_with(|| Accumulated {
            settings: settings.clone(),
            hashes: 0,
            first: now,
            last: now,
            changed: false,
        });
        sample.changed |= sample.settings != settings;
        sample.settings = settings;
        sample.last = now;
        // The first counter predates our observation window; do not count it.
        if !first_sample {
            sample.hashes = sample.hashes.saturating_add(hashes);
        }
    }

    fn drain(&self) -> Vec<BackendSample> {
        let Ok(mut samples) = self.0.lock() else {
            return Vec::new();
        };
        let now = Instant::now();
        let missed = self.1.swap(false, Ordering::Relaxed);
        let result = samples
            .iter()
            .filter_map(|(id, s)| {
                let observed_secs = s.last.duration_since(s.first).as_secs_f64();
                (observed_secs >= 60.0).then(|| BackendSample {
                    instance: *id,
                    settings: s.settings.clone(),
                    hashes: s.hashes,
                    observed_secs,
                    settings_changed: s.changed || missed,
                })
            })
            .collect();
        samples.retain(|_, s| now.duration_since(s.last) < Duration::from_secs(60));
        for s in samples.values_mut() {
            s.hashes = 0;
            // Counters arriving next include work since the last poll.
            s.first = s.last;
            s.changed = false;
        }
        result
    }
}

pub(crate) struct Worker {
    stop: Sender<()>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.stop.try_send(());
        // HTTP has a strict total timeout, so shutdown remains bounded.
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn hardware() -> Hardware {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();
    let identity = crate::runtime_identity::cpu_identity();
    Hardware {
        os: std::env::consts::OS.into(),
        os_version: sysinfo::System::os_version(),
        runtime_environment: crate::runtime_identity::runtime_environment(),
        cpu_model: sys
            .cpus()
            .first()
            .map(|c| c.brand().to_owned())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "unknown".into()),
        cpu_vendor: identity.vendor,
        cpu_family: identity.family,
        cpu_model_id: identity.model,
        logical_cpus: sys.cpus().len() as u32,
        physical_cores: sys.physical_core_count().map(|v| v as u32),
        memory_gib: sys.total_memory().div_ceil(1024 * 1024 * 1024) as u32,
        supported_cpu_kernels: blocknet_pow_kernel::CpuKernel::ALL
            .into_iter()
            .filter(|k| k.is_supported())
            .map(|k| k.to_string())
            .collect(),
    }
}

fn http_client() -> reqwest::Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(3))
        .redirect(reqwest::redirect::Policy::none())
        .build()
}

fn upload(client: &reqwest::blocking::Client, options: &Options, report: &Report) {
    if report.validate() && still_enabled(options) {
        let _ = client.post(&options.endpoint).json(report).send();
    }
}

pub(crate) fn start(options: &Options) -> Result<(Recorder, Worker)> {
    let recorder = Recorder(
        Arc::new(Mutex::new(BTreeMap::new())),
        Arc::new(AtomicBool::new(false)),
    );
    let samples = recorder.clone();
    let options = options.clone();
    let (stop, stopped) = bounded(1);
    let thread = thread::Builder::new()
        .name("hardware-telemetry".into())
        .spawn(move || {
            let Ok(client) = http_client() else {
                return;
            };
            let hardware = hardware();
            let mut since = Instant::now();
            // Wait for productive work, then report hourly. A bounded channel wakes shutdown.
            let mut delay = Duration::from_secs(300);
            loop {
                if stopped.recv_timeout(delay).is_ok() || !still_enabled(&options) {
                    break;
                }
                let backends = samples.drain();
                let window_secs = backends
                    .iter()
                    .fold(since.elapsed().as_secs(), |secs, sample| {
                        secs.max(sample.observed_secs.ceil() as u64)
                    });
                let report = Report {
                    schema_version: SCHEMA_VERSION,
                    installation_id: options.installation_id.clone(),
                    report_id: match random_id() {
                        Ok(id) => id,
                        Err(_) => break,
                    },
                    miner_version: crate::version::RELEASE_VERSION.into(),
                    build_target: crate::runtime_identity::build_target().into(),
                    build_features: {
                        let features = crate::runtime_identity::build_features();
                        if features.is_empty() {
                            "cpu-only".into()
                        } else {
                            features.into()
                        }
                    },
                    debug_build: cfg!(debug_assertions),
                    hardware: hardware.clone(),
                    window_secs,
                    backends,
                };
                since = Instant::now();
                // Failed samples are discarded; no offline spool or retry storms.
                upload(&client, &options, &report);
                delay = INTERVAL;
            }
        })?;
    Ok((
        recorder,
        Worker {
            stop,
            thread: Some(thread),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(
        data_dir: &Path,
        choice: Option<bool>,
        endpoint: Option<&str>,
    ) -> Result<Option<Options>> {
        resolve_with_default(data_dir, choice, endpoint, None)
    }

    #[test]
    fn release_destination_enables_reporting_without_interaction_and_honors_opt_out() {
        let dir = Temp::new();
        let url = Some("https://collector.example/v1/reports");
        assert!(resolve_with_default(&dir.0, None, None, url)
            .unwrap()
            .is_some());
        assert!(resolve_with_default(&dir.0, Some(false), None, url)
            .unwrap()
            .is_none());
        assert!(resolve_with_default(&dir.0, None, None, url)
            .unwrap()
            .is_none());
    }

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            Self(
                std::env::temp_dir().join(format!("seine-telemetry-test-{}", random_id().unwrap())),
            )
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn unconfigured_build_is_inactive_and_configured_build_defaults_on() {
        let dir = Temp::new();
        assert!(resolve(&dir.0, None, None).unwrap().is_none());
        assert!(!dir.0.exists());
        assert!(
            resolve(&dir.0, None, Some("https://collector.example/v1/reports"),)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn opt_out_persists_across_destination_changes() {
        let dir = Temp::new();
        let first = resolve(
            &dir.0,
            Some(true),
            Some("https://collector.example/v1/reports"),
        )
        .unwrap()
        .unwrap();
        assert!(still_enabled(&first));
        let remembered = resolve(&dir.0, None, None).unwrap().unwrap();
        assert_eq!(first.installation_id, remembered.installation_id);
        assert!(resolve(&dir.0, Some(false), None).unwrap().is_none());
        assert!(
            resolve(&dir.0, None, Some("https://other.example/v1/reports"))
                .unwrap()
                .is_none()
        );
        assert!(!still_enabled(&first));
        let other = resolve(&dir.0, Some(true), None).unwrap().unwrap();
        assert_ne!(first.installation_id, other.installation_id);
        assert!(resolve(&dir.0, Some(false), None).unwrap().is_none());
        assert!(!still_enabled(&other));
        fs::write(&first.state_path, "broken").unwrap();
        assert!(resolve(&dir.0, Some(false), None).unwrap().is_none());
    }

    #[test]
    fn changing_settings_marks_sample_unsuitable_for_performance_comparison() {
        let r = Recorder(
            Arc::new(Mutex::new(BTreeMap::new())),
            Arc::new(AtomicBool::new(false)),
        );
        let fixture: Report =
            serde_json::from_str(include_str!("../telemetry/fixture.json")).unwrap();
        let mut settings = fixture.backends[0].settings.clone();
        r.record(1, settings.clone(), 100);
        {
            let mut samples = r.0.lock().unwrap();
            samples.get_mut(&1).unwrap().first = Instant::now() - Duration::from_secs(120);
        }
        settings.lanes = 16;
        r.record(1, settings, 200);
        let report = r.drain();
        assert_eq!(report.len(), 1);
        assert!(report[0].settings_changed);
        assert_eq!(report[0].hashes, 200);
        assert!(r.drain().is_empty());
    }

    #[test]
    fn upload_obeys_opt_out_and_does_not_follow_redirects_or_retry_errors() {
        let dir = Temp::new();
        let server = httpmock::MockServer::start();
        let redirect = server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/report");
            then.status(302).header("Location", server.url("/trap"));
        });
        let trap = server.mock(|when, then| {
            when.path("/trap");
            then.status(204);
        });
        let options = resolve(&dir.0, None, Some(&server.url("/report")))
            .unwrap()
            .unwrap();
        let report: Report =
            serde_json::from_str(include_str!("../telemetry/fixture.json")).unwrap();
        let client = http_client().unwrap();
        upload(&client, &options, &report);
        redirect.assert_hits(1);
        trap.assert_hits(0);
        resolve(&dir.0, Some(false), None).unwrap();
        upload(&client, &options, &report);
        redirect.assert_hits(1);
        let failed = server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/failed");
            then.status(503);
        });
        let options = resolve(&dir.0, Some(true), Some(&server.url("/failed")))
            .unwrap()
            .unwrap();
        upload(&client, &options, &report);
        failed.assert_hits(1);
        let (_, worker) = start(&options).unwrap();
        drop(worker); // Wakes immediately rather than waiting for the reporting interval.
        failed.assert_hits(1);
    }

    #[test]
    fn endpoint_cannot_redirect_to_cleartext_or_embed_secrets() {
        assert!(validate_endpoint("https://telemetry.example/v1/reports").is_ok());
        assert!(validate_endpoint("http://127.0.0.1:9080/v1/reports").is_ok());
        for bad in [
            "http://telemetry.example/v1/reports",
            "https://token@telemetry.example/",
            "https://telemetry.example/?token=secret",
            "file:///tmp/report",
        ] {
            assert!(validate_endpoint(bad).is_err(), "{bad}");
        }
    }
}
