# `epoll_pwait03`

## Goal

Finish the epoll family Phase 3 by enabling `epoll_pwait03`, which relies on
tight timing guarantees from `epoll_pwait()` and `epoll_pwait2()` when sleeping
for microsecond-scale deadlines while measuring the resulting latency.

## Root cause

This testcase previously failed because the kernel wired `ManagedTimeout` to
the coarse `JIFFIES_TIMER_MANAGER`. That manager fires at 1 ms granularity,
which is too coarse for the 1–25 ms threshold that `epoll_pwait03` monitors,
so the measured timings frequently drifted outside the allowable range.
`ManagedTimeout::new()` now uses `MonotonicClock::timer_manager()`, which ticks
in true nanoseconds, eliminating the previous jitter-induced failures.

## Solution

- Re-enable `epoll_pwait03` in `testcases/all.txt`.
- Document the successful triage and link it to the earlier monotonic timer
  manager fix in `family-epoll-02`/`family-epoll-03`.

## Validation

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=epoll_pwait03
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 LTP_CASES=epoll_pwait03
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat LTP_CASES=epoll_pwait03
```

All three targets reported `TPASS` with the measured latencies staying within
the `2450–2540` µs thresholds (even at large sleep values), which matches the
test's expectations.

## Impact / Residual risk

- This change just enables an existing test whose only remaining barrier was
  timer accuracy; no new code paths were touched.
- Keep an eye on other timer-heavy epoll cases in case the kernel's timer
  manager choices diverge in the future.
