# `signal01`

## Goal

Re-enable `signal01` in Phase 3 and verify that it passes on `/tmp`, `/ext2`,
and `/exfat`.

## Root Cause

There was no remaining kernel defect in this case. `signal01` checks the
Linux/POSIX rule that `SIGKILL` cannot be ignored, reset, or caught, and that
the signal still kills the target process with its default action.

The current signal implementation already rejects `signal(SIGKILL, ...)` with
`EINVAL` and still delivers `SIGKILL` with default semantics. The testcase had
simply remained disabled in `testcases/all.txt`.

## Solution

- Re-ran `signal01` on `/tmp`, `/ext2`, and `/exfat`.
- Confirmed that the existing implementation already matches the expected
  `SIGKILL` semantics.
- Re-enabled `signal01` in `testcases/all.txt`.

## Validation

```bash
SSH_PORT=43022 NGINX_PORT=49080 REDIS_PORT=47379 IPERF_PORT=46201 \
LMBENCH_TCP_LAT_PORT=42234 LMBENCH_TCP_BW_PORT=42236 MEMCACHED_PORT=42211 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=signal01

SSH_PORT=43022 NGINX_PORT=49080 REDIS_PORT=47379 IPERF_PORT=46201 \
LMBENCH_TCP_LAT_PORT=42234 LMBENCH_TCP_BW_PORT=42236 MEMCACHED_PORT=42211 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=signal01

SSH_PORT=43022 NGINX_PORT=49080 REDIS_PORT=47379 IPERF_PORT=46201 \
LMBENCH_TCP_LAT_PORT=42234 LMBENCH_TCP_BW_PORT=42236 MEMCACHED_PORT=42211 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=signal01
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed behavior on all three runs:

- `signal(SIGKILL, ...)` returned `EINVAL`
- child processes terminated with default `SIGKILL` handling

## Impact / Residual Risk

- This extends active signal-family negative-path coverage without a kernel
  code change.
- The remaining disabled cases in this family are narrower signal syscall
  coverage items, including the still-missing `rt_sigqueueinfo` path.
