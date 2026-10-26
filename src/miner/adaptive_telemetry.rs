//! Optional, bounded evidence on the tuning thread. No hashing or network I/O.
use crate::backend::cpu::CpuAdaptiveSnapshot;
use crate::config::{Config, CpuAffinityMode, CpuAutotuneResources, CpuPageMode};
use crate::hardware_telemetry::Recorder;
use blocknet_pow_kernel::CpuKernel;
use seine_telemetry::tuning::*;

struct Window {
    started: u64,
    hashes: u64,
    context: TrialContext,
    changed: bool,
    last_hashes: u64,
    last_seen: u64,
}

struct Trace {
    outcome: TrialOutcome,
    window: Option<Window>,
}

pub(super) struct Monitor {
    recorder: Recorder,
    // Align the tuner's existing whole-second clock to this report's run.
    // Do not change the clock used for tuning decisions to collect telemetry.
    origin_ms: u64,
    members: Vec<(TrialAffinity, TrialPageMode)>,
    context: TrialContext,
    ready: bool,
    hashes: u64,
    trace: Option<Trace>,
    pending: Option<(TrialOutcome, TrialArm)>,
}

fn same_context(a: &TrialContext, b: &TrialContext) -> bool {
    a.members == b.members
        && a.resources.allowed_logical_cpus == b.resources.allowed_logical_cpus
        && a.resources.available_parallelism == b.resources.available_parallelism
        && a.resources.effective_memory_mib == b.resources.effective_memory_mib
        && a.resources.placement_fingerprint == b.resources.placement_fingerprint
}

impl Monitor {
    pub(super) fn new(recorder: Recorder, cfg: &Config) -> Self {
        let members = cfg
            .backend_specs
            .iter()
            .filter(|s| s.kind == super::BackendKind::Cpu)
            .map(|s| {
                let affinity = match s.cpu_affinity.unwrap_or(cfg.cpu_affinity) {
                    CpuAffinityMode::Auto => TrialAffinity::Auto,
                    CpuAffinityMode::Os => TrialAffinity::Os,
                    CpuAffinityMode::Off => TrialAffinity::Off,
                    CpuAffinityMode::PcoreOnly => TrialAffinity::PcoreOnly,
                };
                let pages = match cfg.cpu_page_mode {
                    CpuPageMode::Auto => TrialPageMode::Auto,
                    CpuPageMode::Regular => TrialPageMode::Regular,
                    CpuPageMode::Large => TrialPageMode::Large,
                    CpuPageMode::Large1G => TrialPageMode::Large1g,
                };
                (affinity, pages)
            })
            .collect();
        Self {
            origin_ms: recorder.trial_elapsed_ms(),
            recorder,
            members,
            context: TrialContext {
                members: Vec::new(),
                resources: TrialResources {
                    allowed_logical_cpus: None,
                    available_parallelism: None,
                    effective_memory_mib: None,
                    available_memory_mib: None,
                    placement_fingerprint: None,
                },
            },
            ready: false,
            hashes: 0,
            trace: None,
            pending: None,
        }
    }

    fn time(&self, now: u64) -> u64 {
        self.origin_ms.saturating_add(now.saturating_mul(1000))
    }

    pub(super) fn observe(
        &mut self,
        now: u64,
        snapshots: &[CpuAdaptiveSnapshot],
        resources: CpuAutotuneResources,
    ) {
        self.context = TrialContext {
            members: snapshots
                .iter()
                .enumerate()
                .filter(|(_, s)| s.running)
                .filter_map(|(slot, s)| {
                    let (affinity, page_mode) = *self.members.get(slot)?;
                    Some(TrialMember {
                        slot: slot.try_into().ok()?,
                        workers: s.threads.try_into().ok()?,
                        kernel: s.kernel.to_string(),
                        backing_mib: s.backing_bytes.map(|b| b / (1024 * 1024)),
                        affinity,
                        page_mode,
                    })
                })
                .collect(),
            resources: TrialResources {
                // Sizing observes usable parallelism, not the actual CPU mask.
                allowed_logical_cpus: None,
                available_parallelism: u32::try_from(resources.parallelism).ok().filter(|n| *n > 0),
                effective_memory_mib: resources.total_bytes.map(|b| b / (1024 * 1024)),
                available_memory_mib: resources.available_bytes.map(|b| b / (1024 * 1024)),
                placement_fingerprint: None,
            },
        };
        self.ready = !self.context.members.is_empty()
            && snapshots
                .iter()
                .filter(|s| s.running)
                .all(|s| s.kernel_ready);
        self.hashes = snapshots
            .iter()
            .filter(|s| s.running)
            .fold(0u64, |n, s| n.saturating_add(s.hashes));
        if let Some(window) = self.trace.as_mut().and_then(|t| t.window.as_mut()) {
            window.changed |= !same_context(&window.context, &self.context)
                || self.hashes < window.hashes
                || !self.ready;
            if !window.changed {
                window.last_hashes = self.hashes;
                window.last_seen = now;
            }
        }
        if let Some((outcome, target)) = self.pending.take() {
            let expected = match target {
                TrialArm::Baseline => &outcome.baseline,
                TrialArm::Candidate => &outcome.candidate,
            };
            if self.ready && self.context.members == *expected {
                self.emit(outcome, now);
            } else if !self.context.members.is_empty()
                && self
                    .context
                    .members
                    .iter()
                    .map(|m| (m.slot, m.workers, m.backing_mib))
                    .collect::<Vec<_>>()
                    != expected
                        .iter()
                        .map(|m| (m.slot, m.workers, m.backing_mib))
                        .collect::<Vec<_>>()
            {
                let mut aborted = outcome;
                aborted.decision = TrialDecision::Aborted;
                aborted.reason = TrialReason::ContextChanged;
                self.emit(aborted, now);
            } else {
                self.pending = Some((outcome, target));
            }
        }
    }

    pub(super) fn begin(
        &mut self,
        now: u64,
        base: CpuKernel,
        candidate: CpuKernel,
        policy: TrialPolicy,
    ) {
        // An unexpectedly replaced trace is evidence too; never report success.
        self.abort(now, TrialReason::ContextChanged);
        let mut baseline = self.context.members.clone();
        let mut challenger = baseline.clone();
        for m in &mut baseline {
            m.kernel = base.to_string();
        }
        for m in &mut challenger {
            m.kernel = candidate.to_string();
        }
        self.trace = Some(Trace {
            window: None,
            outcome: TrialOutcome {
                kind: TrialKind::CpuKernel,
                policy,
                started_ms: self.time(now),
                finished_ms: self.time(now),
                baseline,
                candidate: challenger,
                context_before: self.context.clone(),
                context_after: self.context.clone(),
                samples: Vec::with_capacity(MAX_SAMPLES),
                decision: TrialDecision::Aborted,
                reason: TrialReason::ContextChanged,
                applied: None,
                cost: TrialCost {
                    switching_ms: None,
                    idle_ms: None,
                },
            },
        });
    }

    pub(super) fn window(&mut self, now: u64) {
        if let Some(trace) = &mut self.trace {
            trace.window = Some(Window {
                started: now,
                hashes: self.hashes,
                context: self.context.clone(),
                changed: false,
                last_hashes: self.hashes,
                last_seen: now,
            });
        }
    }

    pub(super) fn discard_window(&mut self) {
        if let Some(trace) = &mut self.trace {
            trace.window = None;
        }
    }

    pub(super) fn sample(&mut self, hashes: u64, seconds: u64) {
        if let Some(trace) = &mut self.trace {
            let changed = trace.window.take().is_some_and(|w| w.changed);
            if trace.outcome.samples.len() < MAX_SAMPLES {
                trace.outcome.samples.push(TrialSample {
                    arm: if trace.outcome.samples.len() % 3 == 1 {
                        TrialArm::Candidate
                    } else {
                        TrialArm::Baseline
                    },
                    hashes,
                    elapsed_ms: seconds.saturating_mul(1000),
                    settling_ms: None,
                    context: self.context.clone(),
                    changed,
                });
            }
        }
    }

    pub(super) fn finish(
        &mut self,
        now: u64,
        decision: TrialDecision,
        reason: TrialReason,
        target: TrialArm,
    ) {
        if let Some(mut trace) = self.trace.take() {
            trace.outcome.decision = decision;
            trace.outcome.reason = reason;
            let expected = match target {
                TrialArm::Baseline => &trace.outcome.baseline,
                TrialArm::Candidate => &trace.outcome.candidate,
            };
            if self.ready && self.context.members == *expected {
                self.emit(trace.outcome, now);
            } else {
                self.pending = Some((trace.outcome, target));
            }
        }
    }

    pub(super) fn abort(&mut self, now: u64, reason: TrialReason) {
        if let Some(trace) = &mut self.trace {
            if let Some(window) = trace.window.take() {
                if window.last_seen > window.started && trace.outcome.samples.len() < MAX_SAMPLES {
                    trace.outcome.samples.push(TrialSample {
                        arm: if trace.outcome.samples.len() % 3 == 1 {
                            TrialArm::Candidate
                        } else {
                            TrialArm::Baseline
                        },
                        hashes: window.last_hashes - window.hashes,
                        elapsed_ms: (window.last_seen - window.started).saturating_mul(1000),
                        settling_ms: None,
                        context: window.context,
                        changed: window.changed,
                    });
                }
            }
        }
        let outcome = self
            .trace
            .take()
            .map(|t| t.outcome)
            .or_else(|| self.pending.take().map(|p| p.0));
        if let Some(mut outcome) = outcome {
            outcome.decision = TrialDecision::Aborted;
            outcome.reason = reason;
            self.emit(outcome, now);
        }
    }

    pub(super) fn abort_observed(
        &mut self,
        now: u64,
        snapshots: &[CpuAdaptiveSnapshot],
        resources: CpuAutotuneResources,
        reason: TrialReason,
    ) {
        // A control failure can follow a successful request to another CPU
        // backend. Refresh actual settings without acknowledging a pending win.
        let pending = self.pending.take();
        self.observe(now, snapshots, resources);
        self.pending = pending;
        self.abort(now, reason);
    }

    fn emit(&self, mut outcome: TrialOutcome, now: u64) {
        outcome.finished_ms = self.time(now);
        outcome.context_after = self.context.clone();
        outcome.applied = if self.ready && outcome.context_after.members == outcome.baseline {
            Some(TrialArm::Baseline)
        } else if self.ready && outcome.context_after.members == outcome.candidate {
            Some(TrialArm::Candidate)
        } else {
            None
        };
        self.recorder.record_trial(outcome);
    }
}
