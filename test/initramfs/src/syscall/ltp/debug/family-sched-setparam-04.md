# `sched_setparam04`

## Goal

Re-enable `sched_setparam04` in Phase 3 and verify that it passes on `/tmp`,
`/ext2`, and `/exfat`.

## Root Cause

There was no remaining kernel defect for this testcase.

The current scheduler syscall implementation already returned the Linux
compatible error codes that `sched_setparam04` checks, for both the libc
wrapper and the raw syscall path. This testcase had simply stayed disabled.

## Solution

- Re-ran `sched_setparam04` on `/tmp`, `/ext2`, and `/exfat`.
- Confirmed that all expected error paths were already implemented.
- Re-enabled `sched_setparam04` in `testcases/all.txt`.

## Validation

```bash
SSH_PORT=46042 NGINX_PORT=52100 REDIS_PORT=50399 IPERF_PORT=49221 \
LMBENCH_TCP_LAT_PORT=45434 LMBENCH_TCP_BW_PORT=45436 MEMCACHED_PORT=45411 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=sched_setparam04

SSH_PORT=46043 NGINX_PORT=52101 REDIS_PORT=50400 IPERF_PORT=49222 \
LMBENCH_TCP_LAT_PORT=45444 LMBENCH_TCP_BW_PORT=45446 MEMCACHED_PORT=45421 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=sched_setparam04

SSH_PORT=46044 NGINX_PORT=52102 REDIS_PORT=50401 IPERF_PORT=49223 \
LMBENCH_TCP_LAT_PORT=45454 LMBENCH_TCP_BW_PORT=45456 MEMCACHED_PORT=45431 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=sched_setparam04
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed behavior:

- invalid PID returned `EINVAL`
- non-existing PID returned `ESRCH`
- invalid userspace pointer returned `EINVAL` for the libc wrapper path here
- invalid priority returned `EINVAL`

## Impact / Residual Risk

- This removes another disabled Priority A scheduler testcase with no kernel
  code change.
- The next remaining `sched_setparam*` testcase is `sched_setparam05`.
