//! The launch engine: runs a session's items in order, waiting between them.
//!
//! The engine only reports what it can actually know: whether the operating
//! system accepted a launch request. It never claims an application is
//! "running" or "ready".

use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::model::{ItemKind, LaunchItem};

/// Performs the OS-specific work. Implemented by the platform crate, and by
/// fakes in tests.
pub trait Launcher: Send + Sync {
    /// Cheap pre-flight check that the target still exists.
    fn check(&self, kind: &ItemKind) -> Result<(), LaunchError>;
    /// Asks the OS to launch the item. Returns once the request was accepted
    /// or rejected; it does not wait for the program to become ready.
    fn launch(&self, kind: &ItemKind) -> Result<Launched, LaunchError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Launched {
    /// Process id, when the OS told us one.
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LaunchError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("access denied")]
    AccessDenied,
    #[error("the administrator prompt was cancelled")]
    ElevationCancelled,
    #[error("no application is associated with this target")]
    NoHandler,
    #[error("no terminal emulator was found")]
    NoTerminal,
    #[error("blocked: {0}")]
    Blocked(String),
    #[error("this kind of item is not supported on this system")]
    Unsupported,
    #[error("{message} (code {code})")]
    Os { code: i64, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemStatus {
    Pending,
    Launching,
    Launched,
    Failed(LaunchError),
    /// The target was missing before launch, or the run was cancelled first.
    Skipped(SkipReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    Missing(LaunchError),
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunEvent {
    Item {
        index: usize,
        status: ItemStatus,
    },
    /// Waiting after `index` before the next item.
    Waiting {
        index: usize,
        duration: Duration,
    },
    WaitEnded {
        index: usize,
    },
    Finished(RunSummary),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RunSummary {
    pub launched: usize,
    pub failed: usize,
    pub skipped: usize,
    pub cancelled: bool,
}

impl RunSummary {
    pub fn all_ok(&self) -> bool {
        !self.cancelled && self.failed == 0 && self.skipped == 0
    }
}

/// Shared between the UI and a running session: cancel the run, or cut the
/// current wait short.
#[derive(Clone, Default)]
pub struct CancelToken {
    inner: Arc<(Mutex<Flags>, Condvar)>,
}

#[derive(Default)]
struct Flags {
    cancelled: bool,
    skip_wait: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum WaitOutcome {
    Elapsed,
    Skipped,
    Cancelled,
}

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        let (lock, cvar) = &*self.inner;
        lock.lock().unwrap_or_else(|e| e.into_inner()).cancelled = true;
        cvar.notify_all();
    }

    pub fn skip_wait(&self) {
        let (lock, cvar) = &*self.inner;
        lock.lock().unwrap_or_else(|e| e.into_inner()).skip_wait = true;
        cvar.notify_all();
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .cancelled
    }

    fn wait(&self, duration: Duration) -> WaitOutcome {
        let (lock, cvar) = &*self.inner;
        let deadline = Instant::now() + duration;
        let mut flags = lock.lock().unwrap_or_else(|e| e.into_inner());
        flags.skip_wait = false;
        loop {
            if flags.cancelled {
                return WaitOutcome::Cancelled;
            }
            if flags.skip_wait {
                flags.skip_wait = false;
                return WaitOutcome::Skipped;
            }
            let now = Instant::now();
            if now >= deadline {
                return WaitOutcome::Elapsed;
            }
            flags = cvar
                .wait_timeout(flags, deadline - now)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }
}

/// Runs `items` in order on the calling thread, reporting progress through
/// `emit`. Call it from a background thread.
pub fn run(
    items: &[LaunchItem],
    launcher: &dyn Launcher,
    cancel: &CancelToken,
    mut emit: impl FnMut(RunEvent),
) -> RunSummary {
    let mut summary = RunSummary::default();
    let last = items.len().saturating_sub(1);

    for (index, item) in items.iter().enumerate() {
        if cancel.is_cancelled() {
            skip_rest(index, items.len(), &mut summary, &mut emit);
            break;
        }

        match launcher.check(&item.kind) {
            Ok(()) => {}
            // A target that is simply gone is skipped; anything else (a
            // blocked link, an unsupported item) is a real failure.
            Err(missing @ LaunchError::NotFound(_)) => {
                summary.skipped += 1;
                emit(RunEvent::Item {
                    index,
                    status: ItemStatus::Skipped(SkipReason::Missing(missing)),
                });
                continue;
            }
            Err(err) => {
                summary.failed += 1;
                emit(RunEvent::Item {
                    index,
                    status: ItemStatus::Failed(err),
                });
                continue;
            }
        }

        emit(RunEvent::Item {
            index,
            status: ItemStatus::Launching,
        });
        let status = match launcher.launch(&item.kind) {
            Ok(_) => {
                summary.launched += 1;
                ItemStatus::Launched
            }
            Err(e) => {
                summary.failed += 1;
                ItemStatus::Failed(e)
            }
        };
        let launched = status == ItemStatus::Launched;
        emit(RunEvent::Item { index, status });

        // Waiting only makes sense after a successful launch with more to come.
        if launched && index < last && item.wait_after_secs > 0 {
            let duration = Duration::from_secs(u64::from(item.wait_after_secs));
            emit(RunEvent::Waiting { index, duration });
            let outcome = cancel.wait(duration);
            emit(RunEvent::WaitEnded { index });
            if outcome == WaitOutcome::Cancelled {
                skip_rest(index + 1, items.len(), &mut summary, &mut emit);
                break;
            }
        }
    }

    summary.cancelled = cancel.is_cancelled();
    emit(RunEvent::Finished(summary));
    summary
}

fn skip_rest(from: usize, len: usize, summary: &mut RunSummary, emit: &mut impl FnMut(RunEvent)) {
    for index in from..len {
        summary.skipped += 1;
        emit(RunEvent::Item {
            index,
            status: ItemStatus::Skipped(SkipReason::Cancelled),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::thread;

    /// Test-only launcher: targets whose name starts with "missing" fail the
    /// pre-check, "fail" fails to launch, everything else succeeds.
    #[derive(Default)]
    struct FakeLauncher {
        launched: Mutex<Vec<String>>,
    }

    fn target(kind: &ItemKind) -> &str {
        match kind {
            ItemKind::Open { target } => target,
            _ => "",
        }
    }

    impl Launcher for FakeLauncher {
        fn check(&self, kind: &ItemKind) -> Result<(), LaunchError> {
            if target(kind).starts_with("missing") {
                Err(LaunchError::NotFound(target(kind).into()))
            } else if target(kind).starts_with("blocked") {
                Err(LaunchError::Blocked("x:".into()))
            } else {
                Ok(())
            }
        }

        fn launch(&self, kind: &ItemKind) -> Result<Launched, LaunchError> {
            if target(kind).starts_with("fail") {
                return Err(LaunchError::AccessDenied);
            }
            self.launched.lock().unwrap().push(target(kind).into());
            Ok(Launched { pid: Some(1) })
        }
    }

    fn item(name: &str, wait: u32) -> LaunchItem {
        let mut item = LaunchItem::new(
            name,
            ItemKind::Open {
                target: name.into(),
            },
        );
        item.wait_after_secs = wait;
        item
    }

    #[test]
    fn launches_in_order_and_reports_each_step() {
        let launcher = FakeLauncher::default();
        let items = [item("a", 0), item("b", 0), item("c", 0)];
        let mut events = Vec::new();
        let summary = run(&items, &launcher, &CancelToken::new(), |e| events.push(e));

        assert_eq!(*launcher.launched.lock().unwrap(), ["a", "b", "c"]);
        assert_eq!(summary.launched, 3);
        assert!(summary.all_ok());
        assert_eq!(
            events[0],
            RunEvent::Item {
                index: 0,
                status: ItemStatus::Launching
            }
        );
        assert_eq!(events.last(), Some(&RunEvent::Finished(summary)));
    }

    #[test]
    fn failures_and_missing_items_do_not_stop_the_run() {
        let launcher = FakeLauncher::default();
        let items = [item("fail-1", 5), item("missing-2", 5), item("ok", 0)];
        let mut events = Vec::new();
        let summary = run(&items, &launcher, &CancelToken::new(), |e| events.push(e));

        assert_eq!(*launcher.launched.lock().unwrap(), ["ok"]);
        assert_eq!(
            (summary.launched, summary.failed, summary.skipped),
            (1, 1, 1)
        );
        assert!(!summary.all_ok());
        // No waiting after a failed or skipped item.
        assert!(!events.iter().any(|e| matches!(e, RunEvent::Waiting { .. })));
        assert!(events.contains(&RunEvent::Item {
            index: 0,
            status: ItemStatus::Failed(LaunchError::AccessDenied)
        }));
    }

    #[test]
    fn a_rejected_pre_check_is_a_failure_not_a_skip() {
        let launcher = FakeLauncher::default();
        let items = [item("blocked-1", 0), item("ok", 0)];
        let mut events = Vec::new();
        let summary = run(&items, &launcher, &CancelToken::new(), |e| events.push(e));
        assert_eq!(
            (summary.launched, summary.failed, summary.skipped),
            (1, 1, 0)
        );
        assert!(events.contains(&RunEvent::Item {
            index: 0,
            status: ItemStatus::Failed(LaunchError::Blocked("x:".into()))
        }));
    }

    #[test]
    fn no_wait_after_the_last_item() {
        let launcher = FakeLauncher::default();
        let items = [item("a", 0), item("b", 600)];
        let started = Instant::now();
        let mut events = Vec::new();
        run(&items, &launcher, &CancelToken::new(), |e| events.push(e));
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(!events.iter().any(|e| matches!(e, RunEvent::Waiting { .. })));
    }

    #[test]
    fn waits_between_items() {
        let launcher = FakeLauncher::default();
        let items = [item("a", 1), item("b", 0)];
        let started = Instant::now();
        let mut events = Vec::new();
        run(&items, &launcher, &CancelToken::new(), |e| events.push(e));
        assert!(started.elapsed() >= Duration::from_secs(1));
        assert!(events.contains(&RunEvent::Waiting {
            index: 0,
            duration: Duration::from_secs(1)
        }));
    }

    #[test]
    fn skip_wait_continues_immediately() {
        let launcher = Arc::new(FakeLauncher::default());
        let cancel = CancelToken::new();
        let items = vec![item("a", 600), item("b", 0)];
        let (worker_launcher, worker_cancel) = (launcher.clone(), cancel.clone());
        let started = Instant::now();
        let worker = thread::spawn(move || run(&items, &*worker_launcher, &worker_cancel, |_| {}));
        thread::sleep(Duration::from_millis(100));
        cancel.skip_wait();
        let summary = worker.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(summary.launched, 2);
        assert!(!summary.cancelled);
    }

    #[test]
    fn cancel_during_wait_skips_the_rest() {
        let launcher = Arc::new(FakeLauncher::default());
        let cancel = CancelToken::new();
        let items = vec![item("a", 600), item("b", 0), item("c", 0)];
        let (worker_launcher, worker_cancel) = (launcher.clone(), cancel.clone());
        let worker = thread::spawn(move || {
            let mut events = Vec::new();
            let summary = run(&items, &*worker_launcher, &worker_cancel, |e| {
                events.push(e)
            });
            (summary, events)
        });
        thread::sleep(Duration::from_millis(100));
        cancel.cancel();
        let (summary, events) = worker.join().unwrap();
        assert_eq!(*launcher.launched.lock().unwrap(), ["a"]);
        assert_eq!((summary.launched, summary.skipped), (1, 2));
        assert!(summary.cancelled);
        assert!(events.contains(&RunEvent::Item {
            index: 2,
            status: ItemStatus::Skipped(SkipReason::Cancelled)
        }));
    }
}
