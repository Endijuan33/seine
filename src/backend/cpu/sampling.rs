//! Synchronized CPU samples with hashing time separate from arena lifecycle time.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Barrier, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::backend::KernelBenchSample;

pub(super) fn sample<Setup, Hash>(
    lanes: usize,
    duration: Duration,
    warmup: bool,
    shutdown: &AtomicBool,
    setup: Setup,
) -> Result<KernelBenchSample>
where
    Setup: Fn(usize) -> Result<Hash> + Sync,
    Hash: FnMut() -> Result<()>,
{
    let wall_start = Instant::now();
    let ready = Barrier::new(lanes + 1);
    let start = Barrier::new(lanes + 1);
    let finished = Barrier::new(lanes);
    let started_at = OnceLock::<Instant>::new();
    let finished_at = Mutex::new(None::<Instant>);
    let error = Mutex::new(None);
    let hashes = AtomicU64::new(0);

    thread::scope(|scope| {
        for lane in 0..lanes {
            let (setup, ready, start, finished, started_at, finished_at, error, hashes) = (
                &setup,
                &ready,
                &start,
                &finished,
                &started_at,
                &finished_at,
                &error,
                &hashes,
            );
            scope.spawn(move || {
                // Every lane reaches both barriers, including failed or cancelled lanes.
                let mut worker = if shutdown.load(Ordering::Relaxed) {
                    None
                } else {
                    match setup(lane).and_then(|mut hash| {
                        if warmup && !shutdown.load(Ordering::Relaxed) {
                            hash()?;
                        }
                        Ok(hash)
                    }) {
                        Ok(hash) => Some(hash),
                        Err(err) => {
                            *error.lock().unwrap() = Some(err);
                            None
                        }
                    }
                };
                ready.wait();
                start.wait();
                let Some(started_at) = started_at.get().copied() else {
                    return;
                };
                let stop_at = started_at + duration;
                let mut local_hashes = 0u64;
                if let Some(hash) = worker.as_mut() {
                    while Instant::now() < stop_at && !shutdown.load(Ordering::Relaxed) {
                        if let Err(err) = hash() {
                            *error.lock().unwrap() = Some(err);
                            break;
                        }
                        local_hashes = local_hashes.saturating_add(1);
                    }
                }
                // Count completed hashes through the last lane's actual completion,
                // including deadline overrun, but stop timing before its arena drops.
                let ended_at = Instant::now();
                {
                    let mut last = finished_at.lock().unwrap();
                    *last = Some(last.map_or(ended_at, |previous| previous.max(ended_at)));
                }
                hashes.fetch_add(local_hashes, Ordering::Relaxed);
                // Keep every arena resident until all lanes finish: early teardown
                // must not compete with another lane's final measured hash.
                finished.wait();
            });
        }
        ready.wait();
        if error.lock().unwrap().is_none() && !shutdown.load(Ordering::Relaxed) {
            let _ = started_at.set(Instant::now());
        }
        start.wait();
    });

    if let Some(err) = error.into_inner().unwrap() {
        return Err(err);
    }
    let elapsed_secs = started_at
        .get()
        .zip(finished_at.into_inner().unwrap())
        .map_or(0.0, |(start, end)| end.duration_since(*start).as_secs_f64());
    Ok(KernelBenchSample {
        hashes: hashes.load(Ordering::Relaxed),
        elapsed_secs,
        wall_elapsed_secs: wall_start.elapsed().as_secs_f64(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn warmup_and_lifecycle_are_excluded_but_last_hash_overrun_is_timed() {
        struct SlowDrop;
        impl Drop for SlowDrop {
            fn drop(&mut self) {
                thread::sleep(Duration::from_millis(40));
            }
        }
        let calls = AtomicUsize::new(0);
        let sample = sample(
            2,
            Duration::from_millis(1),
            true,
            &AtomicBool::new(false),
            |_| {
                thread::sleep(Duration::from_millis(40));
                let arena = SlowDrop;
                let calls = &calls;
                Ok(move || {
                    let _arena = &arena;
                    calls.fetch_add(1, Ordering::Relaxed);
                    thread::sleep(Duration::from_millis(20));
                    Ok(())
                })
            },
        )
        .unwrap();
        assert_eq!(calls.load(Ordering::Relaxed) as u64, sample.hashes + 2);
        assert!(sample.elapsed_secs >= 0.020);
        assert!(sample.wall_elapsed_secs - sample.elapsed_secs >= 0.090);
    }

    #[test]
    fn failed_setup_or_warmup_releases_every_lane() {
        for fail_warmup in [false, true] {
            let result = sample(
                3,
                Duration::from_millis(1),
                true,
                &AtomicBool::new(false),
                |lane| {
                    if lane == 1 && !fail_warmup {
                        return Err(anyhow!("setup failure"));
                    }
                    Ok(move || {
                        if lane == 1 {
                            Err(anyhow!("warmup failure"))
                        } else {
                            Ok(())
                        }
                    })
                },
            );
            assert!(result.is_err());
        }
    }

    #[test]
    fn shutdown_during_warmup_does_not_start_measurement() {
        let shutdown = AtomicBool::new(false);
        let sample = sample(2, Duration::from_secs(10), true, &shutdown, |_| {
            Ok(|| {
                shutdown.store(true, Ordering::Relaxed);
                Ok(())
            })
        })
        .unwrap();
        assert_eq!(sample.hashes, 0);
        assert_eq!(sample.elapsed_secs, 0.0);
    }

    #[test]
    fn preexisting_shutdown_skips_allocation() {
        let sample = sample(
            2,
            Duration::from_secs(10),
            true,
            &AtomicBool::new(true),
            |_| {
                panic!("must not allocate");
                #[allow(unreachable_code)]
                Ok(|| Ok(()))
            },
        )
        .unwrap();
        assert_eq!(sample.hashes, 0);
        assert_eq!(sample.elapsed_secs, 0.0);
    }
}
