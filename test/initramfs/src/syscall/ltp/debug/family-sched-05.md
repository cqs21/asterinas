# `sched_setscheduler03`

## Goal

Re-enable `sched_setscheduler03` in Phase 3 and verify that it passes on
`/tmp`, `/ext2`, and `/exfat`.

## Root Cause

The testcase checks that an unprivileged task can still switch among
non-real-time scheduling policies when the requested priority is `0`, even
after `RLIMIT_NICE` is tightened. Asterinas already handled `SCHED_OTHER` and
`SCHED_IDLE` correctly in this path, but it rejected `SCHED_BATCH` with
`EINVAL` because the legacy scheduler-policy decoder did not recognize policy
value `3`.

## Solution

- Added user-visible `SCHED_BATCH` decoding in the legacy scheduling attribute
  conversion path.
- Mapped `SCHED_BATCH` to the same internal fair-scheduler policy used for
  non-real-time tasks.
- Re-enabled `sched_setscheduler03` in `testcases/all.txt`.

## Validation

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=sched_setscheduler03

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=sched_setscheduler03

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=sched_setscheduler03
```

Observed result on all three workdirs:

- `sched_setscheduler03` finished with `PASS`
- libc and raw-syscall variants both accepted `SCHED_OTHER`, `SCHED_BATCH`,
  and `SCHED_IDLE` after privilege drop

## Impact / Residual Risk

- This fixes a compatibility gap in the legacy `sched_setscheduler()` policy
  decoder for `SCHED_BATCH`.
- Internally, `SCHED_BATCH` is still represented with the same fair scheduler
  policy as `SCHED_OTHER`; future tests that require round-tripping the exact
  batch policy via getters may need a more explicit internal distinction.
