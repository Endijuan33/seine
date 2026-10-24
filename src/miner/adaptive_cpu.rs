//! Productive CPU tuning: startup uses a seed, and A/B/A trials run on real work.
//! RAM limits constrain the operating count without replacing a remembered peak.
use super::*;
use crate::config::cpu_autotune_capacity;
use std::thread::{self, JoinHandle};

const RECOVERY_SECS: u64 = 30;
const RETRY_SECS: u64 = 300;
const MAX_DRIFT: f64 = 0.08;

pub(super) fn limits(cfg: &Config, owned_bytes: u64) -> (usize, usize, usize) {
    let (capacity, available) = cpu_autotune_capacity(cfg, owned_bytes);
    let max = cfg
        .cpu_autotune_max_threads
        .unwrap_or(capacity)
        .min(capacity)
        .max(1);
    let min = cfg.cpu_autotune_min_threads.max(1).min(max);
    (min, max, available.min(max))
}

fn instances(cfg: &Config) -> usize {
    cfg.backend_specs
        .iter()
        .filter(|s| s.kind == BackendKind::Cpu)
        .count()
}

fn compatible(record: &CpuAutotuneRecord, cfg: &Config, host: &CpuAutotuneHostFingerprint) -> bool {
    // Schema 10 already has warmed scores. Old records remain useful as seeds.
    (10..=CPU_AUTOTUNE_RECORD_SCHEMA_VERSION).contains(&record.schema_version)
        && record.host_fingerprint == *host
        && record.affinity == cpu_affinity_label(cfg.cpu_affinity)
        && record.page_mode == cfg.cpu_page_mode.as_str()
        && record.profile == cpu_profile_label(cfg.cpu_profile)
        && record.cpu_instances == instances(cfg)
        && record.selected_threads > 0
        && record.measured_hps.is_finite()
        && record.measured_hps >= 0.0
}

fn read_seed(cfg: &Config) -> Option<CpuAutotuneRecord> {
    let record: CpuAutotuneRecord =
        serde_json::from_slice(&fs::read(&cfg.cpu_autotune_config_path).ok()?).ok()?;
    compatible(&record, cfg, &cpu_autotune_host_fingerprint()).then_some(record)
}

fn seed_threads(
    record: Option<&CpuAutotuneRecord>,
    min: usize,
    max: usize,
    available: usize,
) -> usize {
    if available == 0 {
        return 0;
    }
    let floor = min.min(available);
    record
        .map(|r| r.selected_threads)
        .unwrap_or((max / 2).max(1))
        .clamp(floor, max.min(available))
}

pub(super) fn prepare(cfg: &mut Config) {
    if instances(cfg) == 0 {
        return;
    }
    let (min, max, available) = limits(cfg, 0);
    let seed = read_seed(cfg);
    let threads = seed_threads(seed.as_ref(), min, max, available);
    // Zero is intentional here. A parked CPU backend stays connected and can
    // recover when a whole arena plus the configured reserve fits again.
    cfg.threads = threads;
    cfg.cpu_auto_threads_cap = max;
    for spec in cfg
        .backend_specs
        .iter_mut()
        .filter(|s| s.kind == BackendKind::Cpu)
    {
        spec.cpu_threads = Some(threads);
    }
    let reason = if available == 0 {
        "waiting for RAM"
    } else if seed.is_some() {
        "cached starting point"
    } else {
        "initial estimate"
    };
    info("MINER", format!("cpu-autotune | starting with {threads} threads ({reason}); tuning while mining, capacity={max}, currently available={available}"));
}

#[derive(Debug, Clone, Copy)]
enum Phase {
    Baseline,
    Probe {
        base: usize,
        before: CpuAutotuneMeasurement,
    },
    Confirm {
        base: usize,
        before: CpuAutotuneMeasurement,
        candidate: usize,
        candidate_sample: CpuAutotuneMeasurement,
    },
}

#[derive(Debug)]
struct Policy {
    current: usize,
    min: usize,
    max: usize,
    profile: CpuPerformanceProfile,
    phase: Phase,
    up: bool,
    attempts: u64,
    ramp: bool,
    next_up_probe: Option<usize>,
    unsuccessful: usize,
    next_trial: u64,
    allocation_retry_at: u64,
    headroom_since: u64,
    previous_cap: usize,
    window_start: u64,
    window_hashes: u64,
    window_secs: u64,
    sampling: bool,
    idle_since: Option<u64>,
    reference_hps: Option<f64>,
    peak_hps: f64,
}

#[derive(Debug)]
struct Decision {
    threads: usize,
    reason: &'static str,
    learned: Option<Vec<(usize, CpuAutotuneMeasurement)>>,
}

impl Policy {
    fn new(cfg: &Config, current: usize, min: usize, max: usize) -> Self {
        Self {
            current,
            min,
            max,
            profile: cfg.cpu_profile,
            phase: Phase::Baseline,
            up: true,
            attempts: 0,
            ramp: true,
            next_up_probe: None,
            unsuccessful: 0,
            next_trial: 0,
            allocation_retry_at: 0,
            headroom_since: 0,
            previous_cap: current,
            window_start: 0,
            window_hashes: 0,
            window_secs: cfg.cpu_autotune_secs.saturating_mul(5).max(30),
            sampling: false,
            idle_since: None,
            reference_hps: None,
            peak_hps: 0.0,
        }
    }

    fn change(&mut self, threads: usize, reason: &'static str) -> Decision {
        self.current = threads;
        self.sampling = false;
        self.idle_since = None;
        Decision {
            threads,
            reason,
            learned: None,
        }
    }

    fn reset(&mut self, now: u64, delay: u64) {
        self.phase = Phase::Baseline;
        self.next_up_probe = None;
        self.sampling = false;
        self.idle_since = None;
        self.next_trial = now.saturating_add(delay);
    }

    fn tick(
        &mut self,
        now: u64,
        hashes: u64,
        ready: bool,
        cap: usize,
        allocation_failed: bool,
        actual: usize,
    ) -> Option<Decision> {
        let cap = cap.min(self.max);
        if cap != self.previous_cap {
            if cap <= self.current || self.previous_cap <= self.current {
                self.headroom_since = now;
            }
            if cap > self.previous_cap {
                self.up = true;
                self.ramp = true;
                self.next_trial = self.next_trial.min(now.saturating_add(RECOVERY_SECS));
            }
            self.previous_cap = cap;
        }
        if allocation_failed || actual < self.current {
            self.reset(now, RETRY_SECS);
            self.allocation_retry_at = now.saturating_add(RETRY_SECS);
            return Some(self.change(actual.min(cap), "allocation limited; retry later"));
        }
        if self.current > cap {
            self.reset(now, RECOVERY_SECS);
            return Some(self.change(cap, "releasing RAM"));
        }
        let comparison_max = match self.phase {
            Phase::Baseline => self.current,
            Phase::Probe { base, .. } => base.max(self.current),
            Phase::Confirm {
                base, candidate, ..
            } => base.max(candidate),
        };
        if comparison_max > cap {
            self.reset(now, RECOVERY_SECS);
            return Some(self.change(
                self.current.min(cap),
                "RAM limit changed; discarding comparison",
            ));
        }
        if self.current == 0 {
            if cap > 0
                && now >= self.next_trial
                && now >= self.allocation_retry_at
                && now.saturating_sub(self.headroom_since) >= RECOVERY_SECS
            {
                self.reset(now, 0);
                return Some(self.change(1, "RAM available; resuming"));
            }
            return None;
        }
        let floor = self.min.min(cap);
        if self.current < floor
            && now >= self.next_trial
            && now >= self.allocation_retry_at
            && now.saturating_sub(self.headroom_since) >= RECOVERY_SECS
        {
            self.reset(now, 0);
            return Some(self.change(floor, "restoring configured minimum after RAM recovery"));
        }
        if !ready {
            if self.sampling {
                // Brief tip changes are part of effective mining throughput.
                // Keep their wall time; a sustained disconnect invalidates the
                // comparison instead of keeping an old baseline indefinitely.
                let idle = self.idle_since.get_or_insert(now);
                if now.saturating_sub(*idle) < 3 {
                    return None;
                }
                let base = match self.phase {
                    Phase::Probe { base, .. } | Phase::Confirm { base, .. } => Some(base),
                    Phase::Baseline => None,
                };
                if let Some(base) = base {
                    self.reset(now, RECOVERY_SECS);
                    return Some(
                        self.change(base.min(cap), "work interrupted; discarding comparison"),
                    );
                }
            }
            self.sampling = false;
            // Interruptions invalidate an A/B/A comparison, but allow a new
            // worker to finish warming before its probe window begins.
            return None;
        }
        self.idle_since = None;
        if !self.sampling {
            self.window_start = now;
            self.window_hashes = hashes;
            self.sampling = true;
            return None;
        }
        let elapsed = now.saturating_sub(self.window_start);
        let completed = hashes.saturating_sub(self.window_hashes);
        if elapsed < self.window_secs || completed < CPU_AUTOTUNE_TARGET_HASHES_PER_CANDIDATE {
            return None;
        }
        let hps = completed as f64 / elapsed.max(1) as f64;
        let sample = CpuAutotuneMeasurement {
            hashes: completed,
            elapsed_secs: elapsed as f64,
            wall_secs: elapsed as f64,
            hps,
        };
        self.sampling = false;
        match self.phase {
            Phase::Baseline => {
                if let Some(old) = self.reference_hps {
                    let ratio = hps / old;
                    if (ratio - 1.0).abs() > MAX_DRIFT {
                        self.next_trial = now;
                        self.up = true;
                        self.ramp = true;
                        self.peak_hps = hps;
                    } else {
                        // Track gradual clock/load drift at the same operating
                        // count without compounding the profile's tolerance.
                        self.peak_hps *= ratio;
                    }
                }
                self.peak_hps = self.peak_hps.max(hps);
                self.reference_hps = Some(hps);
                if now < self.next_trial || now < self.allocation_retry_at {
                    return None;
                }
                let floor = self.min.min(cap);
                // Mostly local probes, with occasional wider probes to escape
                // a plateau. All probes still hash useful pool/daemon work.
                let step = if (self.ramp && self.up) || self.attempts % 6 == 5 {
                    (self.current / 2).max(1)
                } else {
                    1
                };
                let can_grow = now.saturating_sub(self.headroom_since) >= RECOVERY_SECS;
                let farther_probe = self
                    .next_up_probe
                    .take()
                    .filter(|&n| can_grow && n > self.current && n <= cap);
                let candidate = if let Some(threads) = farther_probe {
                    threads
                } else if self.up && can_grow && self.current < cap {
                    self.current.saturating_add(step).min(cap)
                } else if !self.up && self.current > floor {
                    self.current.saturating_sub(step).max(floor)
                } else if can_grow && self.current < cap {
                    self.current + 1
                } else if self.current > floor {
                    self.current - 1
                } else {
                    self.next_trial = now + RETRY_SECS;
                    return Some(Decision {
                        threads: self.current,
                        reason: "only feasible count; continuing to watch resources",
                        learned: Some(vec![(self.current, sample)]),
                    });
                };
                self.attempts += 1;
                self.phase = Phase::Probe {
                    base: self.current,
                    before: sample,
                };
                Some(self.change(candidate, "measuring useful mining work"))
            }
            Phase::Probe { base, before } => {
                self.phase = Phase::Confirm {
                    base,
                    before,
                    candidate: self.current,
                    candidate_sample: sample,
                };
                Some(self.change(base.min(cap), "checking baseline again"))
            }
            Phase::Confirm {
                base,
                before,
                candidate,
                candidate_sample,
            } => {
                let stable = (hps / before.hps - 1.0).abs() <= MAX_DRIFT;
                let baseline = merge_cpu_autotune_measurements(before, sample);
                let previous_peak = self.peak_hps;
                if stable {
                    self.peak_hps = self.peak_hps.max(baseline.hps).max(candidate_sample.hps);
                } else {
                    self.peak_hps = hps;
                }
                let quantization = 1.0 / before.elapsed_secs
                    + 1.0 / sample.elapsed_secs
                    + 1.0 / candidate_sample.elapsed_secs;
                let better = stable
                    && candidate <= cap
                    && if candidate < base {
                        candidate_sample.hps
                            >= self.peak_hps * cpu_autotune_peak_floor_frac(self.profile)
                    } else {
                        candidate_sample.hps > baseline.hps * 1.02 + quantization
                            // Discover the peak initially, then retain fewer
                            // workers when they already satisfy the profile.
                            // Probes still update the peak before this choice.
                            && (self.ramp
                                || baseline.hps
                                    < self.peak_hps * cpu_autotune_peak_floor_frac(self.profile))
                    };
                // A new peak can justify probing farther even if this candidate
                // does not justify a larger operating count. This discovers
                // recovered capacity when load changes leave the baseline's
                // own rate unchanged, without recommitting known higher counts.
                let next_up_probe = (stable
                    && !better
                    && candidate > base
                    && candidate < cap
                    && candidate_sample.hps > previous_peak * 1.02 + quantization)
                    .then(|| candidate + 1);
                self.up = if !stable {
                    true
                } else if better {
                    candidate > base
                } else {
                    candidate < base
                };
                if better || next_up_probe.is_some() {
                    self.unsuccessful = 0;
                } else {
                    self.unsuccessful += 1;
                    self.ramp = !stable;
                }
                self.reset(
                    now,
                    if self.unsuccessful < 2 {
                        RECOVERY_SECS
                    } else {
                        RETRY_SECS
                    },
                );
                self.next_up_probe = next_up_probe;
                let mut decision = self.change(
                    if better { candidate } else { base.min(cap) },
                    if !stable {
                        "load changed; discarding comparison"
                    } else if better {
                        "confirmed better setting"
                    } else {
                        "keeping confirmed setting"
                    },
                );
                self.reference_hps = Some(if better { candidate_sample.hps } else { hps });
                if stable {
                    decision.learned = Some(vec![(base, baseline), (candidate, candidate_sample)]);
                }
                Some(decision)
            }
        }
    }
}

pub(super) struct Controller {
    stop: Sender<()>,
    handle: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct MemoryPressure {
    #[cfg(target_os = "linux")]
    previous: Option<(Instant, u64, u64)>,
}

#[derive(Default)]
struct PressureBackoff {
    consecutive: u8,
}

impl PressureBackoff {
    fn cap(&mut self, pressure: bool, available: usize, current: usize, max: usize) -> usize {
        // First-touch/reclaim can briefly stall while ample clean cache remains
        // available. Require both sustained stalls and tight arena headroom.
        self.consecutive = if pressure && available <= current {
            self.consecutive.saturating_add(1)
        } else {
            0
        };
        if self.consecutive >= 3 {
            available.min(max).min(current.saturating_sub(1))
        } else {
            available.min(max)
        }
    }
}

impl MemoryPressure {
    fn elevated(&mut self) -> bool {
        #[cfg(target_os = "linux")]
        {
            let counters = read_pressure_totals(
                crate::config::linux_cgroup_v2_path().as_deref(),
                std::path::Path::new("/proc/pressure/memory"),
            );
            let Some((some, full)) = counters else {
                self.previous = None;
                return false;
            };
            let now = Instant::now();
            let previous = self.previous.replace((now, some, full));
            previous.is_some_and(|(at, old_some, old_full)| {
                let micros = now.saturating_duration_since(at).as_micros() as u64;
                micros > 0
                    && (some.saturating_sub(old_some) > micros / 20
                        || full.saturating_sub(old_full) > micros / 100)
            })
        }
        #[cfg(target_os = "windows")]
        unsafe {
            use windows_sys::Win32::Foundation::CloseHandle;
            use windows_sys::Win32::System::Memory::{
                CreateMemoryResourceNotification, LowMemoryResourceNotification,
                QueryMemoryResourceNotification,
            };
            let handle = CreateMemoryResourceNotification(LowMemoryResourceNotification);
            if handle.is_null() {
                return false;
            }
            let mut low = 0;
            let ok = QueryMemoryResourceNotification(handle, &mut low);
            CloseHandle(handle);
            ok != 0 && low != 0
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            false
        }
    }
}

#[cfg(any(target_os = "linux", test))]
fn read_pressure_totals(
    cgroup: Option<&std::path::Path>,
    system: &std::path::Path,
) -> Option<(u64, u64)> {
    // A small cgroup budget can coexist with abundant host RAM. System-wide
    // stalls from unrelated tasks must not force this healthy group to shrink.
    cgroup
        .and_then(|dir| fs::read_to_string(dir.join("memory.pressure")).ok())
        .and_then(|raw| pressure_totals(&raw))
        .or_else(|| {
            fs::read_to_string(system)
                .ok()
                .and_then(|raw| pressure_totals(&raw))
        })
}

#[cfg(any(target_os = "linux", test))]
fn pressure_totals(raw: &str) -> Option<(u64, u64)> {
    let total = |kind| {
        raw.lines()
            .find(|line| line.split_whitespace().next() == Some(kind))?
            .split_whitespace()
            .find_map(|word| word.strip_prefix("total="))?
            .parse()
            .ok()
    };
    Some((total("some")?, total("full")?))
}

impl Controller {
    pub(super) fn start(
        cfg: Config,
        backends: Vec<Arc<CpuBackend>>,
        tui: Option<TuiState>,
    ) -> Result<Self> {
        let (stop, rx) = bounded(1);
        let handle = if backends.is_empty() {
            None
        } else {
            Some(
                thread::Builder::new()
                    .name("cpu-autotune".into())
                    .spawn(move || {
                        let (min, max, _) = limits(&cfg, 0);
                        let mut policy = Policy::new(&cfg, cfg.threads, min, max);
                        let mut record = read_seed(&cfg);
                        let host = cpu_autotune_host_fingerprint();
                        let started = Instant::now();
                        let mut failures = 0;
                        let mut pressure = MemoryPressure::default();
                        let mut pressure_backoff = PressureBackoff::default();
                        loop {
                            match rx.recv_timeout(Duration::from_secs(1)) {
                                Ok(()) | Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                                    break
                                }
                                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                            }
                            let all_snapshots: Vec<_> =
                                backends.iter().map(|b| b.adaptive_snapshot()).collect();
                            // A quarantined backend is never restarted by the tuner.
                            let snapshots: Vec<_> =
                                all_snapshots.iter().filter(|s| s.running).collect();
                            if snapshots.is_empty() {
                                break;
                            }
                            let actual = snapshots.iter().map(|s| s.threads).min().unwrap_or(0);
                            let owned = snapshots.iter().map(|s| s.committed_bytes).sum();
                            let hashes = snapshots.iter().map(|s| s.hashes).sum();
                            let count_failures =
                                snapshots.iter().map(|s| s.allocation_failures).sum();
                            let ready = snapshots.iter().all(|s| {
                                s.hashing
                                    && s.warmed == s.threads
                                    && s.resident_bytes == s.threads as u64 * CPU_LANE_MEMORY_BYTES
                            });
                            let (_, available) = cpu_autotune_capacity(&cfg, owned);
                            let cap =
                                pressure_backoff.cap(pressure.elevated(), available, actual, max);
                            if let Some(decision) = policy.tick(
                                started.elapsed().as_secs(),
                                hashes,
                                ready,
                                cap,
                                count_failures > failures,
                                actual,
                            ) {
                                for (backend, snapshot) in backends.iter().zip(&all_snapshots) {
                                    if snapshot.running {
                                        backend.set_active_threads(decision.threads);
                                    }
                                }
                                info(
                                    "MINER",
                                    format!(
                                        "cpu-autotune | {}{} threads per CPU backend: {} (available={available}, effective cap={cap})",
                                        if decision.learned.is_some() {
                                            "done — "
                                        } else {
                                            ""
                                        },
                                        decision.threads,
                                        decision.reason
                                    ),
                                );
                                if let Some(values) = decision
                                    .learned
                                    .filter(|_| snapshots.len() == backends.len())
                                {
                                    let result = save_observation(
                                        &cfg,
                                        &host,
                                        record.as_ref(),
                                        decision.threads,
                                        cap,
                                        max,
                                        &values,
                                    );
                                    match result {
                                        Ok(saved) => record = Some(saved),
                                        Err(err) => warn(
                                            "MINER",
                                            format!(
                                                "cpu-autotune | could not save progress: {err:#}"
                                            ),
                                        ),
                                    }
                                }
                            }
                            failures = count_failures;
                            if let Some(tui) = &tui {
                                if let Ok(mut state) = tui.lock() {
                                    state.threads = policy.current * snapshots.len();
                                    if let Some(control) = &mut state.thread_control {
                                        control.current_threads = policy.current;
                                        control.max_threads = max;
                                    }
                                }
                            }
                        }
                    })?,
            )
        };
        Ok(Self { stop, handle })
    }

    pub(super) fn stop(&mut self) {
        let _ = self.stop.try_send(());
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.stop();
    }
}

fn save_observation(
    cfg: &Config,
    host: &CpuAutotuneHostFingerprint,
    previous: Option<&CpuAutotuneRecord>,
    selected: usize,
    available: usize,
    max: usize,
    values: &[(usize, CpuAutotuneMeasurement)],
) -> Result<CpuAutotuneRecord> {
    let memory_limited = available < max;
    let retained = previous
        .filter(|r| memory_limited && r.selected_threads > available && r.selected_threads <= max);
    let selected_threads = retained.map_or(selected, |r| r.selected_threads);
    let divisor = instances(cfg).max(1) as f64;
    // A retained kernel winner is a count hint, not a measured mining score.
    // Zero denotes an as-yet unmeasured selected count in this mining record.
    let measured_hps = retained
        .filter(|r| r.mining_measurement)
        .map(|r| r.measured_hps)
        .unwrap_or_else(|| {
            values
                .iter()
                .find(|(n, _)| *n == selected_threads)
                .map_or(0.0, |(_, sample)| sample.hps / divisor)
        });
    let mut measurements = previous
        .filter(|r| r.mining_measurement)
        .map(|r| r.measurements.clone())
        .unwrap_or_default();
    for &(n, sample) in values {
        measurements.insert(n, sample);
    }
    let record = CpuAutotuneRecord {
        schema_version: CPU_AUTOTUNE_RECORD_SCHEMA_VERSION,
        profile: cpu_profile_label(cfg.cpu_profile).into(),
        affinity: cpu_affinity_label(cfg.cpu_affinity).into(),
        page_mode: cfg.cpu_page_mode.as_str().into(),
        cpu_instances: instances(cfg),
        min_threads: cfg.cpu_autotune_min_threads,
        max_threads: max,
        selected_threads,
        measured_hps,
        autotune_secs: cfg.cpu_autotune_secs,
        host_fingerprint: host.clone(),
        timestamp_unix_secs: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        measurements,
        memory_limited,
        mining_measurement: true,
    };
    if let Some(parent) = cfg
        .cpu_autotune_config_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let temp = cfg
        .cpu_autotune_config_path
        .with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&temp, serde_json::to_vec_pretty(&record)?)?;
    if let Err(err) = fs::rename(&temp, &cfg.cpu_autotune_config_path) {
        let _ = fs::remove_file(&temp);
        return Err(err.into());
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "runs productive online tuning for up to five minutes with real 2 GiB arenas; run optimized"]
    fn productive_controller_real_smoke() {
        use crate::backend::{NonceChunk, WorkAssignment, WorkTemplate};
        let mut cfg = super::super::tests::test_config();
        cfg.cpu_autotune_threads = true;
        cfg.cpu_autotune_max_threads = Some(2);
        cfg.cpu_autotune_secs = 6;
        cfg.cpu_page_mode = CpuPageMode::Regular;
        cfg.cpu_affinity = crate::config::CpuAffinityMode::Auto;
        let dir = std::env::temp_dir().join(format!(
            "seine-live-tune-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        cfg.cpu_autotune_config_path = dir.join("tuning.json");
        let start = Instant::now();
        prepare(&mut cfg);
        assert_eq!(
            limits(&cfg, 0).2,
            2,
            "live comparison smoke requires capacity for two 2 GiB arenas plus reserve/overhead"
        );
        let backend = Arc::new(CpuBackend::adaptive(
            cfg.threads,
            2,
            cfg.cpu_affinity,
            CpuBackendTuning {
                page_mode: CpuPageMode::Regular,
                ..CpuBackendTuning::default()
            },
        ));
        backend.start().unwrap();
        let mut controller =
            Controller::start(cfg.clone(), vec![Arc::clone(&backend)], None).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            backend
                .assign_work(WorkAssignment {
                    template: Arc::new(WorkTemplate {
                        work_id: 1,
                        epoch: 1,
                        header_base: Arc::from(vec![0; blocknet_pow_spec::POW_HEADER_BASE_LEN]),
                        target: [0; 32],
                        dynamic_share_target: None,
                        pause_on_solution: false,
                        stop_at: Instant::now() + Duration::from_secs(310),
                    }),
                    nonce_chunk: NonceChunk {
                        start_nonce: 0,
                        nonce_count: 1_000_000,
                    },
                })
                .unwrap();
            while !cfg.cpu_autotune_config_path.exists() {
                assert!(
                    start.elapsed() < Duration::from_secs(300),
                    "controller did not confirm a stable comparison"
                );
                std::thread::sleep(Duration::from_millis(100));
            }
            let record = read_seed(&cfg).expect("persisted mining record is compatible");
            assert!(record.mining_measurement);
            assert_eq!(record.measurements.len(), 2);
            assert!(record
                .measurements
                .values()
                .all(|m| m.hashes >= 20 && m.elapsed_secs >= 30.0));
            let elapsed = start.elapsed().as_secs_f64();
            let completed = backend.adaptive_snapshot().hashes;
            // Model a restart. A second miner correctly sees the first miner's
            // still-resident arenas as unavailable and would clamp its seed.
            controller.stop();
            backend.stop();
            let mut cached = cfg.clone();
            let cached_start = Instant::now();
            prepare(&mut cached);
            assert_eq!(cached.threads, record.selected_threads);
            println!("productive_controller_real_smoke elapsed_secs={elapsed:.3} completed_hashes={completed} selected_threads={} cached_prepare_secs={:.4} measurements={:?}", record.selected_threads, cached_start.elapsed().as_secs_f64(), record.measurements);
        }));
        controller.stop();
        backend.stop();
        let _ = fs::remove_dir_all(&dir);
        result.unwrap();
    }

    fn sample(hps: f64) -> CpuAutotuneMeasurement {
        CpuAutotuneMeasurement {
            hashes: (hps * 30.0) as u64,
            elapsed_secs: 30.0,
            wall_secs: 30.0,
            hps,
        }
    }

    fn policy(current: usize) -> Policy {
        Policy::new(&super::super::tests::test_config(), current, 1, 16)
    }

    #[test]
    fn pressure_releases_workers_and_recovery_requires_stable_headroom() {
        let mut p = policy(8);
        let d = p.tick(1, 0, true, 2, false, 8).unwrap();
        assert_eq!(d.threads, 2);
        assert!(d.learned.is_none());
        assert_eq!(p.tick(2, 0, true, 0, false, 2).unwrap().threads, 0);
        assert!(p.tick(3, 0, false, 8, false, 0).is_none());
        assert!(p.tick(32, 0, false, 8, false, 0).is_none());
        assert_eq!(p.tick(33, 0, false, 8, false, 0).unwrap().threads, 1);
    }

    #[test]
    fn allocation_failure_cools_down_even_when_os_reports_headroom() {
        let mut p = policy(4);
        assert_eq!(p.tick(1, 0, false, 16, true, 0).unwrap().threads, 0);
        assert!(p.tick(299, 0, false, 16, false, 0).is_none());
        assert_eq!(p.tick(301, 0, false, 16, false, 0).unwrap().threads, 1);
    }

    #[test]
    fn fluctuating_ram_does_not_shorten_allocation_failure_backoff() {
        for initial in [0, 1] {
            let mut p = policy(4);
            assert_eq!(
                p.tick(1, 0, false, 16, true, initial).unwrap().threads,
                initial
            );
            p.tick(2, 0, false, 2, false, initial);
            p.tick(3, 0, false, 16, false, initial);
            p.min = 4;
            assert!(p.tick(299, 0, false, 16, false, initial).is_none());
            assert_eq!(
                p.tick(301, 0, false, 16, false, initial).unwrap().threads,
                if initial == 0 { 1 } else { 4 }
            );
        }
    }

    #[test]
    fn pressure_backoff_requires_sustained_stalls_and_tight_headroom() {
        let mut p = PressureBackoff::default();
        // A user ceiling of two must not conceal room for ten arenas.
        for _ in 0..10 {
            assert_eq!(p.cap(true, 10, 2, 2), 2);
        }
        for _ in 0..10 {
            assert_eq!(p.cap(true, 3, 2, 8), 3);
        }
        assert_eq!(p.cap(true, 2, 2, 8), 2);
        assert_eq!(p.cap(true, 2, 2, 8), 2);
        assert_eq!(p.cap(false, 2, 2, 8), 2);
        assert_eq!(p.cap(true, 2, 2, 8), 2);
        assert_eq!(p.cap(true, 2, 2, 8), 2);
        assert_eq!(p.cap(true, 2, 2, 8), 1);
        assert_eq!(p.cap(false, 10, 1, 8), 8);
        // The byte budget can shed workers immediately, including all workers.
        assert_eq!(p.cap(false, 1, 8, 16), 1);
        assert_eq!(p.cap(false, 0, 1, 16), 0);
    }

    #[test]
    fn recovered_ram_restores_configured_minimum_before_performance_trials() {
        let mut p = policy(1);
        p.min = 4;
        assert!(p.tick(0, 0, false, 8, false, 1).is_none());
        assert!(p.tick(29, 0, false, 8, false, 1).is_none());
        assert_eq!(p.tick(30, 0, false, 8, false, 1).unwrap().threads, 4);
    }

    #[test]
    fn changed_load_discards_aba_result() {
        let mut p = policy(4);
        p.phase = Phase::Confirm {
            base: 4,
            before: sample(100.0),
            candidate: 6,
            candidate_sample: sample(150.0),
        };
        p.tick(0, 0, true, 16, false, 4);
        let d = p.tick(30, 1800, true, 16, false, 4).unwrap();
        assert_eq!(d.threads, 4);
        assert!(d.learned.is_none());
    }

    #[test]
    fn network_gap_and_new_memory_cap_invalidate_comparisons() {
        for memory_drop in [false, true] {
            let mut p = policy(4);
            p.phase = Phase::Probe {
                base: 8,
                before: sample(100.0),
            };
            p.tick(0, 0, true, 16, false, 4);
            if !memory_drop {
                assert!(p.tick(1, 10, false, 16, false, 4).is_none());
            }
            let d = p
                .tick(
                    4,
                    10,
                    memory_drop,
                    if memory_drop { 6 } else { 16 },
                    false,
                    4,
                )
                .unwrap();
            assert!(d.learned.is_none());
            assert!(matches!(p.phase, Phase::Baseline));
            assert!(d.threads <= if memory_drop { 6 } else { 16 });
        }
    }

    #[test]
    fn productive_trials_find_peak_and_revisit_after_load_changes() {
        let mut p = policy(4);
        let mut hashes = 0;
        let mut learned_high = false;
        let mut learned_low = false;
        let mut recovered = false;
        for now in 0..7200 {
            let peak: usize = if (2400..4800).contains(&now) { 2 } else { 8 };
            let hps = (p.current.min(peak) * 20)
                .saturating_sub(p.current.saturating_sub(peak) * 3)
                .max(1);
            hashes += hps as u64;
            if let Some(d) = p.tick(now, hashes, true, 16, false, p.current) {
                if d.learned.is_some() {
                    learned_high |= now < 2400 && d.threads == 8;
                    learned_low |= (2400..4800).contains(&now) && d.threads == 2;
                    recovered |= now >= 4800 && d.threads == 8;
                }
            }
        }
        assert!(learned_high, "must explore above startup count");
        assert!(learned_low, "must revisit under sustained competing load");
        assert!(
            recovered,
            "a busy interval must not permanently cap throughput"
        );
    }

    #[test]
    fn efficiency_keeps_qualifying_count_across_sustained_rechecks() {
        let mut p = policy(4);
        p.max = 8;
        p.profile = CpuPerformanceProfile::Efficiency;
        let rates = [0, 24, 48, 72, 96, 112, 124, 132, 136];
        let mut hashes = 0;
        let mut confirmations = 0;
        for now in 0..14_400 {
            hashes += rates[p.current];
            if let Some(d) = p.tick(now, hashes, true, 8, false, p.current) {
                if now >= 1800 && d.learned.is_some() {
                    confirmations += 1;
                    assert_eq!(d.threads, 5, "steady-state confirmation at {now}s");
                }
            }
        }
        assert!(
            confirmations >= 20,
            "exercise repeated rechecks, not one endpoint"
        );
    }

    #[test]
    fn efficiency_rediscovers_peak_after_cpu_load_changes() {
        let mut p = policy(4);
        p.max = 8;
        p.profile = CpuPerformanceProfile::Efficiency;
        let normal = [0, 24, 48, 72, 96, 112, 124, 132, 136];
        let loaded = [0, 24, 48, 64, 60, 56, 52, 48, 44];
        let mut hashes = 0;
        let mut confirmations = [0; 3];
        for now in 0..14_400 {
            let stage = (now / 4800) as usize;
            hashes += if stage == 1 {
                loaded[p.current]
            } else {
                normal[p.current]
            };
            if let Some(d) = p.tick(now, hashes, true, 8, false, p.current) {
                if now % 4800 >= 1800 && d.learned.is_some() {
                    confirmations[stage] += 1;
                    assert_eq!(
                        d.threads,
                        if stage == 1 { 2 } else { 5 },
                        "stage {stage}, {now}s"
                    );
                }
            }
        }
        assert!(confirmations.iter().all(|&count| count >= 5));
    }

    #[test]
    fn efficiency_rediscovers_peak_after_ram_capacity_returns() {
        let mut p = policy(2);
        p.max = 8;
        p.profile = CpuPerformanceProfile::Efficiency;
        let rates = [0, 24, 48, 72, 96, 112, 124, 132, 136];
        let mut hashes = 0;
        let mut recovered = 0;
        for now in 0..8000 {
            let cap = if now < 4000 { 2 } else { 8 };
            hashes += rates[p.current];
            if let Some(d) = p.tick(now, hashes, true, cap, false, p.current) {
                if now >= 6000 && d.learned.is_some() {
                    recovered += 1;
                    assert_eq!(d.threads, 5, "recovered capacity at {now}s");
                }
            }
        }
        assert!(recovered >= 5);
    }

    #[test]
    fn efficiency_tolerance_does_not_compound_across_downward_steps() {
        let mut p = policy(8);
        p.profile = CpuPerformanceProfile::Efficiency;
        let mut hashes = 0;
        let mut last_confirmed = 8;
        for now in 0..5000 {
            hashes += p.current.min(8) as u64 * 20;
            if let Some(d) = p.tick(now, hashes, true, 16, false, p.current) {
                if d.learned.is_some() {
                    last_confirmed = d.threads;
                }
            }
        }
        assert_eq!(
            last_confirmed, 6,
            "retain 75% of the measured peak, not 75% per step"
        );
    }

    #[test]
    fn pressure_prefers_current_cgroup_and_falls_back_when_unavailable() {
        let dir = std::env::temp_dir().join(format!(
            "seine-pressure-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let system = dir.join("system.pressure");
        let group = dir.join("memory.pressure");
        fs::write(&system, "some total=100000\nfull total=10000\n").unwrap();
        fs::write(&group, "some total=10\nfull total=1\n").unwrap();
        assert_eq!(read_pressure_totals(Some(&dir), &system), Some((10, 1)));
        assert_eq!(read_pressure_totals(None, &system), Some((100000, 10000)));
        fs::write(&group, "unavailable").unwrap();
        assert_eq!(
            read_pressure_totals(Some(&dir), &system),
            Some((100000, 10000))
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn pressure_parser_requires_both_stall_totals() {
        assert_eq!(
            pressure_totals("some avg10=0.10 avg60=0.10 total=1000\nfull avg10=0 total=20\n"),
            Some((1000, 20))
        );
        assert_eq!(pressure_totals("some total=1000\n"), None);
        assert_eq!(pressure_totals("some total=bad\nfull total=20\n"), None);
    }

    #[test]
    fn cached_start_respects_current_ram_and_explicit_bounds() {
        let cfg = super::super::tests::test_config();
        let r = record(&cfg, 12);
        assert_eq!(seed_threads(Some(&r), 1, 16, 16), 12);
        assert_eq!(seed_threads(Some(&r), 1, 16, 3), 3);
        assert_eq!(seed_threads(Some(&r), 1, 16, 0), 0);
        assert_eq!(seed_threads(Some(&r), 1, 6, 16), 6);
        assert_eq!(seed_threads(None, 8, 16, 3), 3);
        assert_eq!(seed_threads(None, 1, 16, 16), 8);
    }

    fn record(cfg: &Config, selected_threads: usize) -> CpuAutotuneRecord {
        CpuAutotuneRecord {
            schema_version: 10,
            profile: cpu_profile_label(cfg.cpu_profile).into(),
            affinity: cpu_affinity_label(cfg.cpu_affinity).into(),
            page_mode: cfg.cpu_page_mode.as_str().into(),
            cpu_instances: instances(cfg),
            min_threads: 1,
            max_threads: 16,
            selected_threads,
            measured_hps: 10.0,
            autotune_secs: 6,
            host_fingerprint: CpuAutotuneHostFingerprint::default(),
            timestamp_unix_secs: 1,
            measurements: BTreeMap::new(),
            memory_limited: false,
            mining_measurement: false,
        }
    }

    #[test]
    fn seed_compatibility_ignores_age_and_transient_bounds_but_keeps_hardware_contract() {
        let mut cfg = super::super::tests::test_config();
        let r = record(&cfg, 12);
        cfg.cpu_auto_threads_cap = 3;
        cfg.cpu_autotune_max_threads = Some(6);
        assert!(compatible(&r, &cfg, &r.host_fingerprint));
        cfg.cpu_page_mode = CpuPageMode::Regular;
        assert!(!compatible(&r, &cfg, &r.host_fingerprint));
        cfg.cpu_page_mode = CpuPageMode::Auto;
        let mut other_host = r.host_fingerprint.clone();
        other_host.affinity_topology = "changed".into();
        assert!(!compatible(&r, &cfg, &other_host));
    }

    #[test]
    fn constrained_progress_keeps_previous_higher_setting_on_disk() {
        let mut cfg = super::super::tests::test_config();
        let path = std::env::temp_dir().join(format!(
            "seine-adaptive-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        cfg.cpu_autotune_config_path = path.clone();
        let previous = record(&cfg, 12);
        let saved = save_observation(
            &cfg,
            &previous.host_fingerprint,
            Some(&previous),
            3,
            4,
            16,
            &[(3, sample(3.0)), (4, sample(2.9))],
        )
        .unwrap();
        let parsed: CpuAutotuneRecord = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!(saved.selected_threads, 12);
        assert_eq!(parsed.selected_threads, 12);
        assert!(parsed.memory_limited);
        assert_eq!(
            parsed.measured_hps, 0.0,
            "do not relabel a kernel score as a mining score"
        );
        assert!(parsed.measurements.contains_key(&3));
        assert_eq!(seed_threads(Some(&parsed), 1, 16, 16), 12);
    }
}
