//! Progress and cancellation for long analyses.
//!
//! The command layer starts a job around each analysis with [`begin`]. The long loops of the analyses (bootstrap and
//! permutation resamples, trees of a forest or boosting, starts of a mixture model, imputations, MCMC iterations) call
//! [`step`] once per pass: it records how far the work has come and answers whether to stop. A stopped analysis breaks out
//! of its loop and the command layer throws its result away, so a cancelled run never adds anything to the output.
//!
//! Another thread (the window, while the analysis runs) reads [`running`] to show progress and calls [`cancel`].
//! Loops outside a job (tests, the staleness re-runs) are never stopped.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;

struct State {
    id: u64,
    label: String,
    cancel: AtomicBool,
    done: AtomicU64,
    total: AtomicU64,
    started: Instant,
}

static NEXT: AtomicU64 = AtomicU64::new(1);
static RUNNING: Mutex<Vec<Arc<State>>> = Mutex::new(Vec::new());

thread_local! {
    static CURRENT: RefCell<Option<Arc<State>>> = const { RefCell::new(None) };
}

/// A running job. It ends when dropped.
pub struct Job(Arc<State>);

impl Job {
    pub fn id(&self) -> u64 {
        self.0.id
    }

    /// Asks the job to stop at its next step.
    pub fn cancel(&self) {
        self.0.cancel.store(true, Ordering::Relaxed);
    }

    pub fn cancelled(&self) -> bool {
        self.0.cancel.load(Ordering::Relaxed)
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        if let Ok(mut list) = RUNNING.lock() {
            list.retain(|s| !Arc::ptr_eq(s, &self.0));
        }
        CURRENT.with(|c| {
            let mut c = c.borrow_mut();
            if c.as_ref().is_some_and(|s| Arc::ptr_eq(s, &self.0)) {
                *c = None;
            }
        });
    }
}

/// Starts a job on this thread; the loops this thread runs until the job is dropped report to it.
pub fn begin(label: &str) -> Job {
    let state = Arc::new(State { id: NEXT.fetch_add(1, Ordering::Relaxed), label: label.to_string(), cancel: AtomicBool::new(false), done: AtomicU64::new(0), total: AtomicU64::new(0), started: Instant::now() });
    if let Ok(mut list) = RUNNING.lock() {
        list.push(state.clone());
    }
    CURRENT.with(|c| *c.borrow_mut() = Some(state.clone()));
    Job(state)
}

/// Records that `done` of `total` passes are finished and answers whether to stop (true when the job was cancelled).
pub fn step(done: usize, total: usize) -> bool {
    CURRENT.with(|c| match c.borrow().as_ref() {
        Some(s) => {
            s.done.store(done as u64, Ordering::Relaxed);
            s.total.store(total as u64, Ordering::Relaxed);
            s.cancel.load(Ordering::Relaxed)
        }
        None => false,
    })
}

/// Whether this thread's job was cancelled.
pub fn cancelled() -> bool {
    CURRENT.with(|c| c.borrow().as_ref().is_some_and(|s| s.cancel.load(Ordering::Relaxed)))
}

/// Cancels the job with this id, or every running job; returns how many were asked to stop.
pub fn cancel(id: Option<u64>) -> usize {
    let Ok(list) = RUNNING.lock() else { return 0 };
    let mut n = 0;
    for s in list.iter().filter(|s| id.is_none_or(|i| s.id == i)) {
        s.cancel.store(true, Ordering::Relaxed);
        n += 1;
    }
    n
}

/// A running job as the window shows it.
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub id: u64,
    pub label: String,
    /// Passes finished of `total`; total is 0 until the analysis reaches a loop that counts.
    pub done: u64,
    pub total: u64,
    pub seconds: f64,
    pub cancelling: bool,
}

pub fn running() -> Vec<Status> {
    let Ok(list) = RUNNING.lock() else { return Vec::new() };
    list.iter()
        .map(|s| Status { id: s.id, label: s.label.clone(), done: s.done.load(Ordering::Relaxed), total: s.total.load(Ordering::Relaxed), seconds: s.started.elapsed().as_secs_f64(), cancelling: s.cancel.load(Ordering::Relaxed) })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_report_and_cancel_stops() {
        let job = begin("test job");
        assert!(!step(3, 10));
        let me = running().into_iter().find(|s| s.id == job.id()).unwrap();
        assert_eq!((me.done, me.total, me.label.as_str()), (3, 10, "test job"));
        assert_eq!(cancel(Some(job.id())), 1);
        assert!(step(4, 10) && cancelled() && job.cancelled());
        drop(job);
        assert!(!step(1, 2), "outside a job nothing stops");
        assert!(running().iter().all(|s| s.label != "test job"));
    }
}
