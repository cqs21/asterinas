# `sched_getaffinity01`

## Goal

Re-enable `sched_getaffinity01` in Phase 3 and verify that it passes on `/tmp`,
`/ext2`, and `/exfat`.

## Root Cause

There was no remaining kernel bug for this testcase.

The current `sched_getaffinity()` implementation already matched the behavior
that LTP checks:

- returning a valid CPU mask for the current system topology
- reporting `EFAULT` for an invalid userspace pointer
- reporting `EINVAL` for an invalid cpuset size
- reporting `ESRCH` for a non-existing thread

This testcase had only remained disabled because the scheduler affinity family
had not yet been re-validated after the recent Priority A work.

## Solution

- Re-ran `sched_getaffinity01` on `/tmp`, `/ext2`, and `/exfat`.
- Confirmed that the existing implementation already satisfies all LTP
  expectations on each target.
- Re-enabled `sched_getaffinity01` in `testcases/all.txt`.

## Validation

```bash
SSH_PORT=46054 NGINX_PORT=52112 REDIS_PORT=50411 IPERF_PORT=49233 \
LMBENCH_TCP_LAT_PORT=45554 LMBENCH_TCP_BW_PORT=45556 MEMCACHED_PORT=45531 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=sched_getaffinity01

SSH_PORT=46055 NGINX_PORT=52113 REDIS_PORT=50412 IPERF_PORT=49234 \
LMBENCH_TCP_LAT_PORT=45564 LMBENCH_TCP_BW_PORT=45566 MEMCACHED_PORT=45541 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=sched_getaffinity01

SSH_PORT=46056 NGINX_PORT=52114 REDIS_PORT=50413 IPERF_PORT=49235 \
LMBENCH_TCP_LAT_PORT=45574 LMBENCH_TCP_BW_PORT=45576 MEMCACHED_PORT=45551 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=sched_getaffinity01
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed behavior:

- the testcase observed a cpuset size of `128` bytes and one enabled CPU on
  this single-vCPU configuration
- the invalid-pointer, invalid-size, and non-existing-thread checks already
  returned `EFAULT`, `EINVAL`, and `ESRCH`

## Impact / Residual Risk

- This extends active scheduler affinity coverage without additional kernel
  code changes.
- The next remaining disabled Priority A scheduler cases are the
  `sched_{set,get}attr*` tests.
