# `sched_setscheduler02`

## Goal

Re-enable `sched_setscheduler02` in Phase 3 and verify that it passes on
`/tmp`, `/ext2`, and `/exfat`.

## Root Cause

The kernel accepted an unprivileged transition to a real-time scheduling
policy. `sched_setscheduler()` updated the target thread's policy directly
after basic argument parsing, but it did not enforce the Linux permission
rules for `SCHED_FIFO` and `SCHED_RR`.

As a result, after LTP dropped the effective UID from `root` to `nobody`,

- `sched_setscheduler(0, SCHED_FIFO, { .sched_priority = 1 })`

still succeeded instead of failing with `EPERM`.

## Solution

- Added permission checks to `sys_sched_setscheduler()`.
- Rejected unprivileged transitions to real-time policy when:
  - the caller lacks `CAP_SYS_NICE`, and
  - `RLIMIT_RTPRIO` does not permit the requested operation.
- Kept same-owner checks for non-capability callers before changing another
  thread's scheduling policy.
- Re-enabled `sched_setscheduler02` in `testcases/all.txt`.

## Validation

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=sched_setscheduler02

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=sched_setscheduler02

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=sched_setscheduler02
```

Observed result on all three workdirs:

- `sched_setscheduler02` finished with `PASS`
- both libc and raw-syscall variants returned `EPERM` as expected

## Impact / Residual Risk

- This fixes the missing real-time scheduling permission check for the legacy
  `sched_setscheduler()` syscall path.
- `sched_setscheduler03` still needs separate validation because it exercises
  non-real-time policy changes and `RLIMIT_NICE` behavior.
