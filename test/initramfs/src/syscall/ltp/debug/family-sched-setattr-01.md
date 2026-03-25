# `sched_setattr01`

## Goal

Re-enable `sched_setattr01` in Phase 3 and verify that it passes on `/tmp`,
`/ext2`, and `/exfat`.

## Root Cause

The kernel rejected the `SCHED_DEADLINE` input used by LTP with `EINVAL`
before it could reach the target-thread lookup.

This testcase checks four paths:

- a valid `sched_setattr()` call on the current thread
- an invalid PID that should fail with `ESRCH`
- a `NULL` attribute pointer that should fail with `EINVAL`
- an invalid `size` field that should fail with `EINVAL`

The last two paths were already correct. The first two failed because
`LinuxSchedAttr -> SchedPolicy` only accepted fair, idle, and real-time
policies. LTP uses a minimal `SCHED_DEADLINE` attribute, so Asterinas returned
`EINVAL` immediately for both the current-thread call and the nonexistent PID
case that Linux reports as `ESRCH`.

## Solution

- Added a `SchedPolicy::Deadline { runtime, deadline, period }` variant to
  preserve the Linux scheduler ABI state needed by `sched_{set,get}attr`.
- Accepted `SCHED_DEADLINE` in `sched_getattr.rs` when the LTP parameter
  invariants hold: nonzero runtime/deadline/period and
  `runtime <= deadline <= period`.
- Serialized `SchedPolicy::Deadline` back to Linux `sched_attr`, so the kernel
  can round-trip the values for follow-up `sched_getattr()` coverage.
- Mapped deadline tasks onto the existing real-time scheduler internals with a
  FIFO policy and the lowest RT priority as a compatibility layer, rather than
  attempting a full EDF implementation.
- Reported zero priority and zero RR interval for deadline tasks in the
  Linux-facing helper syscalls.
- Re-enabled `sched_setattr01` in `testcases/all.txt`.

## Validation

```bash
SSH_PORT=46058 NGINX_PORT=52116 REDIS_PORT=50415 IPERF_PORT=49237 \
LMBENCH_TCP_LAT_PORT=45594 LMBENCH_TCP_BW_PORT=45596 MEMCACHED_PORT=45571 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=sched_setattr01

SSH_PORT=46059 NGINX_PORT=52117 REDIS_PORT=50416 IPERF_PORT=49238 \
LMBENCH_TCP_LAT_PORT=45604 LMBENCH_TCP_BW_PORT=45606 MEMCACHED_PORT=45581 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=sched_setattr01

SSH_PORT=46060 NGINX_PORT=52118 REDIS_PORT=50417 IPERF_PORT=49239 \
LMBENCH_TCP_LAT_PORT=45614 LMBENCH_TCP_BW_PORT=45616 MEMCACHED_PORT=45591 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=sched_setattr01
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed behavior:

- before the fix, the first subcheck failed with `EINVAL` instead of success
- before the fix, the invalid PID subcheck failed with `EINVAL` instead of
  `ESRCH`
- after the fix, LTP observed `SUCCESS`, `ESRCH`, and the two expected
  `EINVAL` results

## Impact / Residual Risk

- This unblocks the first Priority A `sched_{set,get}attr*` testcase.
- The implementation is intentionally an ABI-compatibility layer for current
  LTP coverage, not a full deadline scheduler.
- `sched_getattr01` and `sched_getattr02` should be validated next, because
  they exercise the same ABI surface more deeply.
