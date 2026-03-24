# `sched_getscheduler01`

## Goal

Re-enable `sched_getscheduler01` in Phase 3 and verify that it passes on
`/tmp`, `/ext2`, and `/exfat`.

## Root Cause

This testcase was not blocked by a kernel bug. It was simply still commented
out in the LTP enable list even though Asterinas already returned the expected
scheduler policies for the combinations exercised by the test.

During validation, LTP also queried `CONFIG_RT_GROUP_SCHED`, but that probe was
informational in this environment and did not affect the testcase result.

## Solution

- Re-ran `sched_getscheduler01` on `/tmp`, `/ext2`, and `/exfat`.
- Confirmed that both libc and raw-syscall variants returned the expected
  policies in all three workdirs.
- Re-enabled the testcase in `testcases/all.txt`.

## Validation

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=sched_getscheduler01

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=sched_getscheduler01

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=sched_getscheduler01
```

Observed result on all three workdirs:

- `sched_getscheduler01` finished with `PASS`
- expected policies `SCHED_RR`, `SCHED_OTHER`, and `SCHED_FIFO` were reported
  correctly in both libc and syscall variants

## Impact / Residual Risk

- No kernel code changes were needed for this testcase.
- The remaining `sched_*` cases may still require real scheduler semantics
  fixes; this result only confirms that the read-only policy query path covered
  by `sched_getscheduler01` already matches LTP expectations.
