// SPDX-License-Identifier: MPL-2.0

use core::sync::atomic::{AtomicU64, Ordering};

use aster_bigtcp::iface::ScheduleNextPoll;
use ostd::sync::WaitQueue;

pub struct PollScheduler {
    /// The time when we should do the next poll.
    /// We store the total number of milliseconds since the system booted.
    next_poll_at_ms: AtomicU64,
    /// The wait queue that the background polling thread will sleep on.
    polling_wait_queue: WaitQueue,
}

impl PollScheduler {
    const INACTIVE: u64 = u64::MAX;

    pub(super) fn new() -> Self {
        Self {
            next_poll_at_ms: AtomicU64::new(Self::INACTIVE),
            polling_wait_queue: WaitQueue::new(),
        }
    }

    pub(super) fn next_poll_at_ms(&self) -> Option<u64> {
        match self.next_poll_at_ms.load(Ordering::Acquire) {
            Self::INACTIVE => None,
            millis => Some(millis),
        }
    }

    pub(super) fn polling_wait_queue(&self) -> &WaitQueue {
        &self.polling_wait_queue
    }
}

impl ScheduleNextPoll for PollScheduler {
    fn schedule_next_poll(&self, poll_at: Option<u64>) {
        let new_val = poll_at.unwrap_or(Self::INACTIVE);
        let old_val = self.next_poll_at_ms.swap(new_val, Ordering::Release);

        let should_wake = match (old_val, new_val) {
            (_, Self::INACTIVE) => false,
            (Self::INACTIVE, _) => true,
            (old, new) => new < old,
        };

        if should_wake {
            self.polling_wait_queue.wake_all();
        }
    }
}
