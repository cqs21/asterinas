# `sched_rr_get_interval01`

## Goal

Re-enable `sched_rr_get_interval01` in Phase 3 and verify that it passes on
`/tmp`, `/ext2`, and `/exfat`.

## Root Cause

This testcase was blocked by a real kernel compatibility gap. Asterinas did not
expose `sched_rr_get_interval` in either syscall dispatch table, so both the
libc path and the raw old-kernel-spec path observed the syscall as unsupported.

Even after wiring the entry, LTP still needs a Linux-compatible time quantum to
be written back to userspace. Without that, the testcase reports an invalid
zero-length interval.

## Solution

- Added a new `sched_rr_get_interval` syscall handler.
- Wired the handler into both the x86 and generic syscall dispatch tables.
- Reused the existing scheduler policy state to return a Linux-compatible
  quantum:
  `15ms` for `SCHED_RR`, `0` for `SCHED_FIFO`, and a positive fair-scheduler
  interval for non-RT policies.
- Re-enabled `sched_rr_get_interval01` in `testcases/all.txt`.

## Validation

```bash
SSH_PORT=46028 NGINX_PORT=52086 REDIS_PORT=50385 IPERF_PORT=49207 \
LMBENCH_TCP_LAT_PORT=45294 LMBENCH_TCP_BW_PORT=45296 MEMCACHED_PORT=45271 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=sched_rr_get_interval01

SSH_PORT=46031 NGINX_PORT=52089 REDIS_PORT=50388 IPERF_PORT=49210 \
LMBENCH_TCP_LAT_PORT=45324 LMBENCH_TCP_BW_PORT=45326 MEMCACHED_PORT=45301 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=sched_rr_get_interval01

SSH_PORT=46032 NGINX_PORT=52090 REDIS_PORT=50389 IPERF_PORT=49211 \
LMBENCH_TCP_LAT_PORT=45334 LMBENCH_TCP_BW_PORT=45336 MEMCACHED_PORT=45311 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=sched_rr_get_interval01
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed behavior on all three runs:

- the libc path succeeded
- the old-kernel-spec raw syscall path succeeded
- the returned time quantum was `0s 15000000ns`

## Impact / Residual Risk

- This closes the missing `sched_rr_get_interval` syscall gap and unlocks
  scheduler quantum coverage for both libc and raw syscall entry points.
- The same kernel change is expected to help the remaining disabled
  `sched_rr_get_interval02` and `sched_rr_get_interval03` cases, which should
  now be triaged next.
