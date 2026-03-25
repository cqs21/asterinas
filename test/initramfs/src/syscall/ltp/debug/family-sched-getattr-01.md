# `sched_getattr01`

## Goal

Re-enable `sched_getattr01` in Phase 3 and verify that it passes on `/tmp`,
`/ext2`, and `/exfat`.

## Root Cause

This testcase had no separate kernel bug after `sched_setattr01` was fixed.

LTP writes a `SCHED_DEADLINE` attribute with `sched_setattr()`, then reads it
back with `sched_getattr()` and checks that the policy and deadline fields are
preserved exactly. Before the previous fix, Asterinas rejected
`SCHED_DEADLINE` at `sched_setattr()` time, so this testcase could not pass.

Once the kernel accepted `SCHED_DEADLINE` and serialized the same values back
through `sched_getattr()`, the testcase passed unchanged.

## Solution

- Revalidated `sched_getattr01` after the `sched_setattr01` compatibility work.
- Confirmed that no additional kernel change was required.
- Re-enabled `sched_getattr01` in `testcases/all.txt`.

## Validation

```bash
SSH_PORT=46061 NGINX_PORT=52119 REDIS_PORT=50418 IPERF_PORT=49240 \
LMBENCH_TCP_LAT_PORT=45624 LMBENCH_TCP_BW_PORT=45626 MEMCACHED_PORT=45601 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=sched_getattr01

SSH_PORT=46062 NGINX_PORT=52120 REDIS_PORT=50419 IPERF_PORT=49241 \
LMBENCH_TCP_LAT_PORT=45634 LMBENCH_TCP_BW_PORT=45636 MEMCACHED_PORT=45611 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=sched_getattr01

SSH_PORT=46063 NGINX_PORT=52121 REDIS_PORT=50420 IPERF_PORT=49242 \
LMBENCH_TCP_LAT_PORT=45644 LMBENCH_TCP_BW_PORT=45646 MEMCACHED_PORT=45621 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=sched_getattr01
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed behavior:

- LTP reported that the attributes written by `sched_setattr()` were read back
  correctly in all three workdirs
- no filesystem-specific behavior difference was observed

## Impact / Residual Risk

- This removes the second-to-last Priority A scheduler testcase from the todo
  list.
- The remaining scheduler Priority A item is `sched_getattr02`, which focuses
  on error handling rather than successful deadline attribute round-trips.
