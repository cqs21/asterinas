# `sched_rr_get_interval03`

## Goal

Re-enable `sched_rr_get_interval03` in Phase 3 and verify that it passes on
`/tmp`, `/ext2`, and `/exfat`.

## Root Cause

There was no new kernel bug in this testcase.

The `sched_rr_get_interval` implementation added for
`sched_rr_get_interval01` already returns Linux-compatible results for the
error paths exercised by `sched_rr_get_interval03`. This testcase remained
disabled only because the family had not been re-validated after the syscall
support landed.

## Solution

- Re-ran `sched_rr_get_interval03` on `/tmp`, `/ext2`, and `/exfat`.
- Confirmed that the current syscall implementation already satisfies both the
  libc path and the raw old-kernel-spec syscall path.
- Re-enabled `sched_rr_get_interval03` in `testcases/all.txt`.

## Validation

```bash
SSH_PORT=46036 NGINX_PORT=52094 REDIS_PORT=50393 IPERF_PORT=49215 \
LMBENCH_TCP_LAT_PORT=45374 LMBENCH_TCP_BW_PORT=45376 MEMCACHED_PORT=45351 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=sched_rr_get_interval03

SSH_PORT=46037 NGINX_PORT=52095 REDIS_PORT=50394 IPERF_PORT=49216 \
LMBENCH_TCP_LAT_PORT=45384 LMBENCH_TCP_BW_PORT=45386 MEMCACHED_PORT=45361 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=sched_rr_get_interval03

SSH_PORT=46038 NGINX_PORT=52096 REDIS_PORT=50395 IPERF_PORT=49217 \
LMBENCH_TCP_LAT_PORT=45394 LMBENCH_TCP_BW_PORT=45396 MEMCACHED_PORT=45371 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=sched_rr_get_interval03
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed behavior:

- invalid PID checks returned the expected `EINVAL` and `ESRCH`
- the raw old-kernel-spec syscall path also returned the expected `EFAULT`

## Impact / Residual Risk

- The `sched_rr_get_interval*` Priority A trio is now fully enabled.
- Remaining Priority A work should move to the next disabled scheduler family
  testcase rather than this syscall.
