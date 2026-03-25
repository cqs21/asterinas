# `sched_rr_get_interval02`

## Goal

Re-enable `sched_rr_get_interval02` in Phase 3 and verify that it passes on
`/tmp`, `/ext2`, and `/exfat`.

## Root Cause

There was no additional kernel defect beyond the `sched_rr_get_interval`
compatibility gap already fixed for `sched_rr_get_interval01`.

Once the syscall was wired into the dispatch tables and started returning a
Linux-compatible interval, `sched_rr_get_interval02` passed unchanged on all
three workdirs. This testcase had simply remained disabled in
`testcases/all.txt`.

## Solution

- Re-ran `sched_rr_get_interval02` on `/tmp`, `/ext2`, and `/exfat`.
- Confirmed that the existing `sched_rr_get_interval` implementation added for
  `sched_rr_get_interval01` already satisfies this testcase.
- Re-enabled `sched_rr_get_interval02` in `testcases/all.txt`.

## Validation

```bash
SSH_PORT=46033 NGINX_PORT=52091 REDIS_PORT=50390 IPERF_PORT=49212 \
LMBENCH_TCP_LAT_PORT=45344 LMBENCH_TCP_BW_PORT=45346 MEMCACHED_PORT=45321 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=sched_rr_get_interval02

SSH_PORT=46034 NGINX_PORT=52092 REDIS_PORT=50391 IPERF_PORT=49213 \
LMBENCH_TCP_LAT_PORT=45354 LMBENCH_TCP_BW_PORT=45356 MEMCACHED_PORT=45331 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=sched_rr_get_interval02

SSH_PORT=46035 NGINX_PORT=52093 REDIS_PORT=50392 IPERF_PORT=49214 \
LMBENCH_TCP_LAT_PORT=45364 LMBENCH_TCP_BW_PORT=45366 MEMCACHED_PORT=45341 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=sched_rr_get_interval02
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed behavior on all three runs:

- the libc path succeeded
- the old-kernel-spec raw syscall path succeeded

## Impact / Residual Risk

- This extends active `sched_rr_get_interval*` coverage without another kernel
  code change.
- The remaining disabled testcase in this trio is `sched_rr_get_interval03`,
  which should be triaged next against the same syscall implementation.
