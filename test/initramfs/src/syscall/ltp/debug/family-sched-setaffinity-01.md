# `sched_setaffinity01`

## Goal

Re-enable `sched_setaffinity01` in Phase 3 and verify that it passes on `/tmp`,
`/ext2`, and `/exfat`.

## Root Cause

The kernel accepted `sched_setaffinity()` on another thread without checking
Linux ownership rules.

LTP exercises four error paths:

- invalid userspace pointer -> `EFAULT`
- empty CPU mask -> `EINVAL`
- non-existing thread -> `ESRCH`
- unprivileged affinity change on another thread -> `EPERM`

The first three were already correct. The last one failed because
`sys_sched_setaffinity()` directly stored the new CPU mask for any existing
target thread and had no permission gate at all.

## Solution

- Added a Linux-compatible permission check before changing another thread's
  affinity.
- Allowed the operation when the target belongs to the same process, when the
  caller effective UID matches the target real/effective UID, or when the
  caller has `CAP_SYS_NICE` in the target thread's user namespace.
- Returned `EPERM` when none of those conditions hold.
- Re-enabled `sched_setaffinity01` in `testcases/all.txt`.

## Validation

```bash
SSH_PORT=46051 NGINX_PORT=52109 REDIS_PORT=50408 IPERF_PORT=49230 \
LMBENCH_TCP_LAT_PORT=45524 LMBENCH_TCP_BW_PORT=45526 MEMCACHED_PORT=45501 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=sched_setaffinity01

SSH_PORT=46052 NGINX_PORT=52110 REDIS_PORT=50409 IPERF_PORT=49231 \
LMBENCH_TCP_LAT_PORT=45534 LMBENCH_TCP_BW_PORT=45536 MEMCACHED_PORT=45511 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=sched_setaffinity01

SSH_PORT=46053 NGINX_PORT=52111 REDIS_PORT=50410 IPERF_PORT=49232 \
LMBENCH_TCP_LAT_PORT=45544 LMBENCH_TCP_BW_PORT=45546 MEMCACHED_PORT=45521 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=sched_setaffinity01
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed behavior:

- before the fix, the final subcheck unexpectedly succeeded when LTP expected
  `EPERM`
- after the fix, the testcase reported `EFAULT`, `EINVAL`, `ESRCH`, and
  `EPERM` for the four intended failure cases

## Impact / Residual Risk

- This fixes another Linux-compatible scheduler permission bug in Priority A.
- `sched_getaffinity01` should be checked next, because it shares the same data
  path but validates read-side affinity semantics instead of write-side
  permissions.
