# `sched_getscheduler02`

## Goal

Re-enable `sched_getscheduler02` in Phase 3 and verify that it passes on
`/tmp`, `/ext2`, and `/exfat`.

## Root Cause

This testcase was also already satisfied by the existing scheduler query path.
It checks that `sched_getscheduler()` returns `ESRCH` for a non-existent PID,
and Asterinas already matched that behavior for both libc and raw-syscall
variants.

The testcase had remained disabled only because it had not yet been revalidated
and re-enabled in the LTP allowlist.

## Solution

- Re-ran `sched_getscheduler02` on `/tmp`, `/ext2`, and `/exfat`.
- Confirmed that both variants returned `ESRCH` for the invalid PID in all
  three workdirs.
- Re-enabled the testcase in `testcases/all.txt`.

## Validation

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=sched_getscheduler02

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=sched_getscheduler02

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=sched_getscheduler02
```

Observed result on all three workdirs:

- `sched_getscheduler02` finished with `PASS`
- `sched_getscheduler(2147483647)` returned `ESRCH` in both libc and syscall
  variants

## Impact / Residual Risk

- No kernel code changes were needed for this testcase.
- The write-side scheduler tests (`sched_setscheduler*`) are separate work and
  may still require semantic fixes even though the error-reporting read path
  already matches Linux here.
