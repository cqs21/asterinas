# `rt_sigtimedwait01`

## Goal

Re-enable `rt_sigtimedwait01` in Phase 3 and verify that it passes on `/tmp`,
`/ext2`, and `/exfat`.

## Root Cause

There was no remaining kernel defect in this case. `rt_sigtimedwait01` checks
the core realtime signal wait path, including successful wakeup on matching
signals, timeout handling, `siginfo_t` contents, restoration of the signal
mask, and `EFAULT` on bad user pointers.

The current `rt_sigtimedwait` implementation already satisfies those
expectations on all tested workdirs. The testcase had simply remained disabled
in `testcases/all.txt`.

## Solution

- Re-ran `rt_sigtimedwait01` on `/tmp`, `/ext2`, and `/exfat`.
- Confirmed that the existing implementation already matches the expected
  Linux-compatible behavior.
- Re-enabled `rt_sigtimedwait01` in `testcases/all.txt`.

## Validation

```bash
SSH_PORT=43022 NGINX_PORT=49080 REDIS_PORT=47379 IPERF_PORT=46201 \
LMBENCH_TCP_LAT_PORT=42234 LMBENCH_TCP_BW_PORT=42236 MEMCACHED_PORT=42211 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=rt_sigtimedwait01

SSH_PORT=43022 NGINX_PORT=49080 REDIS_PORT=47379 IPERF_PORT=46201 \
LMBENCH_TCP_LAT_PORT=42234 LMBENCH_TCP_BW_PORT=42236 MEMCACHED_PORT=42211 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=rt_sigtimedwait01

SSH_PORT=43022 NGINX_PORT=49080 REDIS_PORT=47379 IPERF_PORT=46201 \
LMBENCH_TCP_LAT_PORT=42234 LMBENCH_TCP_BW_PORT=42236 MEMCACHED_PORT=42211 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=rt_sigtimedwait01
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed behavior on all three runs:

- matching signals woke the waiter correctly
- timeout paths returned as expected
- `siginfo_t` contents matched LTP's checks
- bad user pointers returned `EFAULT`
- the original signal mask was restored after the wait

## Impact / Residual Risk

- This extends active realtime-signal wait coverage without a kernel code
  change.
- The remaining disabled realtime-signal work in this family is centered on
  `rt_sigqueueinfo`, which currently lacks syscall table support.
