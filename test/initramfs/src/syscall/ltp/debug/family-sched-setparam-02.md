# `sched_setparam02`

## Goal

Re-enable `sched_setparam02` in Phase 3 and verify that it passes on `/tmp`,
`/ext2`, and `/exfat`.

## Root Cause

There was no remaining kernel bug for this testcase.

The scheduler parameter behavior already matched what LTP expects for both the
libc wrapper and raw syscall paths. `sched_setparam02` was still disabled only
because the family had not been re-validated after earlier scheduler fixes.

## Solution

- Re-ran `sched_setparam02` on `/tmp`, `/ext2`, and `/exfat`.
- Confirmed that the kernel already reports the expected priorities in all
  subchecks.
- Re-enabled `sched_setparam02` in `testcases/all.txt`.

## Validation

```bash
SSH_PORT=46039 NGINX_PORT=52097 REDIS_PORT=50396 IPERF_PORT=49218 \
LMBENCH_TCP_LAT_PORT=45404 LMBENCH_TCP_BW_PORT=45406 MEMCACHED_PORT=45381 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=sched_setparam02

SSH_PORT=46040 NGINX_PORT=52098 REDIS_PORT=50397 IPERF_PORT=49219 \
LMBENCH_TCP_LAT_PORT=45414 LMBENCH_TCP_BW_PORT=45416 MEMCACHED_PORT=45391 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=sched_setparam02

SSH_PORT=46041 NGINX_PORT=52099 REDIS_PORT=50398 IPERF_PORT=49220 \
LMBENCH_TCP_LAT_PORT=45424 LMBENCH_TCP_BW_PORT=45426 MEMCACHED_PORT=45401 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=sched_setparam02
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed behavior:

- the libc path returned the expected priorities `5`, `5`, and `0`
- the raw syscall path returned the same expected priorities

## Impact / Residual Risk

- This extends active scheduler Priority A coverage without additional kernel
  code changes.
- The remaining disabled `sched_setparam*` cases should be triaged next.
