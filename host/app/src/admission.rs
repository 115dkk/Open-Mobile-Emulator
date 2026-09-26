// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// One native operation at a time; rejected callers never join a work queue.
#[derive(Clone, Debug, Default)]
pub(crate) struct CommandAdmission(Arc<AtomicBool>);

impl CommandAdmission {
    pub(crate) fn try_enter(&self) -> Option<CommandLease> {
        self.0
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| CommandLease(Arc::clone(&self.0)))
    }
}

/// Ownership follows the blocking task, even if its async caller is cancelled.
#[derive(Debug)]
pub(crate) struct CommandLease(Arc<AtomicBool>);

impl Drop for CommandLease {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::CommandAdmission;

    #[test]
    fn clones_reject_work_until_the_slot_is_released() {
        let admission = CommandAdmission::default();
        let clone = admission.clone();
        let lease = admission.try_enter().expect("initial admission");
        for _ in 0..1024 {
            assert!(clone.try_enter().is_none());
        }
        drop(lease);
        assert!(clone.try_enter().is_some());
    }

    #[test]
    fn blocking_task_keeps_the_slot_after_its_caller_leaves() {
        let admission = CommandAdmission::default();
        let lease = admission.try_enter().expect("initial admission");
        let (release, wait) = std::sync::mpsc::channel();
        let task = std::thread::spawn(move || {
            let _lease = lease;
            wait.recv().expect("test releases task");
        });
        assert!(admission.try_enter().is_none());
        release.send(()).expect("task is waiting");
        task.join().expect("task completes");
        assert!(admission.try_enter().is_some());
    }

    #[test]
    fn unwind_releases_the_slot() {
        let admission = CommandAdmission::default();
        let result = std::panic::catch_unwind(|| {
            let _lease = admission.try_enter().expect("initial admission");
            panic!("test operation panicked");
        });
        assert!(result.is_err());
        assert!(admission.try_enter().is_some());
    }
}
