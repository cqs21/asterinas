# `rt_sigqueueinfo01`

## Goal

Re-enable `rt_sigqueueinfo01` in Phase 3 and verify that it passes on `/tmp`,
`/ext2`, and `/exfat`.

## Root Cause

This testcase was blocked by a real kernel compatibility gap. The guest kernel
did not expose an `rt_sigqueueinfo` syscall entry in the architecture dispatch
tables, so LTP treated the syscall as unsupported and reported `TCONF` instead
of executing the signal delivery path.

## Solution

- Added a new `rt_sigqueueinfo` syscall handler.
- Wired the handler into both the x86 and generic syscall dispatch tables.
- Implemented the Linux-compatible behavior needed by LTP:
  process/thread-group target resolution, user `siginfo_t` ingestion with the
  kernel overriding `si_signo`, and `EPERM` rejection for invalid `si_code`
  values when signaling another process.
- Re-enabled `rt_sigqueueinfo01` in `testcases/all.txt`.

## Validation

```bash
SSH_PORT=44022 NGINX_PORT=50080 REDIS_PORT=48379 IPERF_PORT=47201 \
LMBENCH_TCP_LAT_PORT=43234 LMBENCH_TCP_BW_PORT=43236 MEMCACHED_PORT=43211 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=rt_sigqueueinfo01

SSH_PORT=44022 NGINX_PORT=50080 REDIS_PORT=48379 IPERF_PORT=47201 \
LMBENCH_TCP_LAT_PORT=43234 LMBENCH_TCP_BW_PORT=43236 MEMCACHED_PORT=43211 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=rt_sigqueueinfo01

SSH_PORT=44022 NGINX_PORT=50080 REDIS_PORT=48379 IPERF_PORT=47201 \
LMBENCH_TCP_LAT_PORT=43234 LMBENCH_TCP_BW_PORT=43236 MEMCACHED_PORT=43211 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=rt_sigqueueinfo01
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed behavior:

- the queued signal was delivered successfully
- the receiver observed the expected signal number and payload value

## Impact / Residual Risk

- This closes a missing realtime signal syscall gap and unlocks user-supplied
  queued signal coverage in LTP.
- The implementation currently resolves non-leader thread IDs to their owning
  process for `rt_sigqueueinfo`, which is sufficient for the current LTP
  coverage but may need refinement if future tests distinguish exact Linux
  thread-group targeting subtleties.
