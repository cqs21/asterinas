# `sched_setscheduler01`

## Goal

Re-enable `sched_setscheduler01` in Phase 3 and verify that it passes on
`/tmp`, `/ext2`, and `/exfat`.

## Root Cause

This testcase was already satisfied by the existing `sched_setscheduler()`
error paths. It exercises invalid-target and invalid-argument handling rather
than successful realtime policy transitions:

- nonexistent PID must return `ESRCH`
- invalid policy must return `EINVAL`
- bad userspace pointer must return `EFAULT`
- invalid priority for the selected policy must return `EINVAL`

Asterinas already matched those expectations in both libc and raw-syscall
variants. The testcase had simply not yet been revalidated and re-enabled in
the LTP allowlist.

## Solution

- Re-ran `sched_setscheduler01` on `/tmp`, `/ext2`, and `/exfat`.
- Confirmed that all expected errno paths were returned in both libc and
  syscall variants on all three workdirs.
- Re-enabled the testcase in `testcases/all.txt`.

## Validation

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=sched_setscheduler01

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=sched_setscheduler01

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=sched_setscheduler01
```

Observed result on all three workdirs:

- `sched_setscheduler01` finished with `PASS`
- expected `ESRCH`, `EINVAL`, and `EFAULT` paths all matched LTP

## Impact / Residual Risk

- No kernel code changes were needed for this testcase.
- The remaining `sched_setscheduler02` and `sched_setscheduler03` cases still
  need separate validation because they cover different scheduler semantics.
