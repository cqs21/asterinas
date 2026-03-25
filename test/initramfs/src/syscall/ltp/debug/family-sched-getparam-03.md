# `sched_getparam03`

## Goal

Re-enable `sched_getparam03` in Phase 3 and verify that it passes on `/tmp`,
`/ext2`, and `/exfat`.

## Root Cause

There was no remaining kernel defect in this case. `sched_getparam03` checks
the error paths of `sched_getparam()` for a non-existing PID, an invalid
negative PID, and an invalid userspace pointer.

The current Asterinas implementation already returns the Linux-compatible
results expected by LTP on all three workdirs:

- `ESRCH` for a non-existing PID
- `EINVAL` for an invalid PID
- `EINVAL` for an invalid `sched_param` pointer

The testcase had simply remained disabled in `testcases/all.txt`.

## Solution

- Re-ran `sched_getparam03` on `/tmp`, `/ext2`, and `/exfat`.
- Confirmed that the existing `sched_getparam()` implementation already
  matches the expected Linux-compatible behavior.
- Re-enabled `sched_getparam03` in `testcases/all.txt`.

## Validation

```bash
SSH_PORT=46022 NGINX_PORT=52080 REDIS_PORT=50379 IPERF_PORT=49201 \
LMBENCH_TCP_LAT_PORT=45234 LMBENCH_TCP_BW_PORT=45236 MEMCACHED_PORT=45211 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=sched_getparam03

SSH_PORT=46023 NGINX_PORT=52081 REDIS_PORT=50380 IPERF_PORT=49202 \
LMBENCH_TCP_LAT_PORT=45244 LMBENCH_TCP_BW_PORT=45246 MEMCACHED_PORT=45221 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=sched_getparam03

SSH_PORT=46025 NGINX_PORT=52083 REDIS_PORT=50382 IPERF_PORT=49204 \
LMBENCH_TCP_LAT_PORT=45264 LMBENCH_TCP_BW_PORT=45266 MEMCACHED_PORT=45241 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=sched_getparam03
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed behavior on all three runs:

- non-existing PID returned `ESRCH`
- invalid PID returned `EINVAL`
- invalid userspace pointer returned `EINVAL`

## Impact / Residual Risk

- This extends active scheduler error-path coverage without a kernel code
  change.
- The remaining disabled `sched*` backlog is now concentrated in affinity,
  `sched_attr`, `sched_rr_get_interval`, and selected `sched_setparam` cases.
