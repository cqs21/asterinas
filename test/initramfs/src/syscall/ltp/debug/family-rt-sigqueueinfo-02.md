# `rt_sigqueueinfo02`

## Goal

Re-enable `rt_sigqueueinfo02` in Phase 3 and verify that it passes on `/tmp`,
`/ext2`, and `/exfat`.

## Root Cause

This testcase was blocked by the same missing `rt_sigqueueinfo` syscall support
as `rt_sigqueueinfo01`. Without a syscall entry, LTP stopped at `TCONF` and
never exercised the required errno behavior for invalid arguments.

## Solution

- Added the `rt_sigqueueinfo` syscall handler and dispatch wiring.
- Implemented the errno paths needed by this testcase:
  `EINVAL` for an invalid signal number, `EPERM` for an invalid `si_code` when
  signaling another process, and `ESRCH` when no matching target thread group
  exists.
- Re-enabled `rt_sigqueueinfo02` in `testcases/all.txt`.

## Validation

```bash
SSH_PORT=44022 NGINX_PORT=50080 REDIS_PORT=48379 IPERF_PORT=47201 \
LMBENCH_TCP_LAT_PORT=43234 LMBENCH_TCP_BW_PORT=43236 MEMCACHED_PORT=43211 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=rt_sigqueueinfo02

SSH_PORT=44022 NGINX_PORT=50080 REDIS_PORT=48379 IPERF_PORT=47201 \
LMBENCH_TCP_LAT_PORT=43234 LMBENCH_TCP_BW_PORT=43236 MEMCACHED_PORT=43211 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=rt_sigqueueinfo02

SSH_PORT=44022 NGINX_PORT=50080 REDIS_PORT=48379 IPERF_PORT=47201 \
LMBENCH_TCP_LAT_PORT=43234 LMBENCH_TCP_BW_PORT=43236 MEMCACHED_PORT=43211 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=rt_sigqueueinfo02
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed errno behavior:

- invalid signal number: `EINVAL`
- invalid `uinfo->si_code` against another process: `EPERM`
- missing target thread group: `ESRCH`

## Impact / Residual Risk

- This extends realtime signal negative-path coverage with a concrete kernel
  implementation instead of a testcase skip.
- The same syscall path will likely be reusable for the related
  `rt_tgsigqueueinfo` work, but that thread-directed variant still needs its
  own implementation and validation.
