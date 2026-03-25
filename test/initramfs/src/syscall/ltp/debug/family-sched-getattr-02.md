# `sched_getattr02`

## Goal

Re-enable `sched_getattr02` in Phase 3 and verify that it passes on `/tmp`,
`/ext2`, and `/exfat`.

## Root Cause

This testcase also needed no new kernel fix after the earlier
`sched_setattr01` / `sched_getattr01` work.

`sched_getattr02` only checks error handling:

- nonexistent PID -> `ESRCH`
- invalid userspace pointer -> `EINVAL`
- undersized `sched_attr` buffer -> `EINVAL`
- unsupported flags -> `EINVAL`

All four paths were already Linux-compatible in the current tree, so once the
remaining scheduler attr cases were revisited, this testcase passed as-is.

## Solution

- Revalidated the existing `sched_getattr()` error paths on all three workdirs.
- Confirmed that no code change was required for this testcase.
- Re-enabled `sched_getattr02` in `testcases/all.txt`.

## Validation

```bash
SSH_PORT=46064 NGINX_PORT=52122 REDIS_PORT=50421 IPERF_PORT=49243 \
LMBENCH_TCP_LAT_PORT=45654 LMBENCH_TCP_BW_PORT=45656 MEMCACHED_PORT=45631 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=sched_getattr02

SSH_PORT=46065 NGINX_PORT=52123 REDIS_PORT=50422 IPERF_PORT=49244 \
LMBENCH_TCP_LAT_PORT=45664 LMBENCH_TCP_BW_PORT=45666 MEMCACHED_PORT=45641 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=sched_getattr02

SSH_PORT=46066 NGINX_PORT=52124 REDIS_PORT=50423 IPERF_PORT=49245 \
LMBENCH_TCP_LAT_PORT=45674 LMBENCH_TCP_BW_PORT=45676 MEMCACHED_PORT=45651 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=sched_getattr02
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed behavior:

- all four expected error codes matched Linux in each workdir
- no filesystem-specific divergence was observed

## Impact / Residual Risk

- This clears the remaining Priority A scheduler testcase backlog.
- Follow-up work should move to the next unfinished Priority A family in
  `all.txt`; the scheduler attr surface is no longer blocking Phase 3.
