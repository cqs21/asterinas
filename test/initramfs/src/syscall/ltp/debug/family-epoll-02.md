# `epoll_wait02`

## Goal

Ensure `epoll_wait02` runs reliably under the LTP `/tmp`, `/ext2`, and `/exfat`
workloads so we can leave the timer-heavy epoll wait test enabled in the
default `testcases/all.txt`.

## Root cause

`epoll_wait02` exercises short relative timeouts. The kernel previously wired
`WaitTimeout::ManagedTimeout::new()` to the coarse `JIFFIES_TIMER_MANAGER`. When
a user request waited relative to that manager, the pending timer could easily
be armed near the end of the current jiffy and fire as soon as the next tick
began, which is ~1ms earlier than intended. That aligned poorly with LTP's
microsecond thresholds and caused the testcase to report premature completions.

## Solution

- Default `ManagedTimeout::new()` to `MonotonicClock::timer_manager()`, the same
  timer manager already powering `CLOCK_MONOTONIC`/`CLOCK_BOOTTIME` users.
  This manager ticks in real nanoseconds and does not suffer the coarse tail
  shot that the `JIFFIES_TIMER_MANAGER` exhibits.
- Enable `epoll_wait02` in `testcases/all.txt` so it runs when the suite is
  executed without an explicit case list.

## Validation

The testcase now passes across the three LTP workdirs:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=epoll_wait02
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 LTP_CASES=epoll_wait02
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat LTP_CASES=epoll_wait02
```

## Impact / Residual risk

- All other waits that relied on the jiffy manager now use the monotonic timer
  manager too, so they also benefit from finer resolution. There should be no
  correctness regression because monotonic is already the clock that exposes
  the wall time that users expect; the change just removes the artificial
  coarse granularity that was leftover from the legacy manager. Continue
  watching other timeout-heavy tests for unexpected latency once more testcases
  enable this path.
