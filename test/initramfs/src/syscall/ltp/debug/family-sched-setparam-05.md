# `sched_setparam05`

## Goal

Re-enable `sched_setparam05` in Phase 3 and verify that it passes on `/tmp`,
`/ext2`, and `/exfat`.

## Root Cause

The kernel accepted scheduler parameter changes that Linux rejects with
`EPERM`.

Two permission bugs were involved:

- `sys_sched_setparam()` changed the target thread priority without reusing the
  permission gate already present in `sched_setscheduler`.
- The shared scheduler ownership check was too permissive and treated a match
  on the caller's real UID as sufficient. For this LTP case, Linux permission
  semantics are based on the caller's effective UID matching the target's real
  or effective UID.

Because of that, unprivileged `sched_setparam()` calls on another thread
incorrectly succeeded instead of failing with `EPERM`.

## Solution

- Reused the `sched_setscheduler` target lookup and permission check from
  `sys_sched_setparam()`.
- Built the new scheduling policy from the target thread's existing policy
  before applying the permission gate, so real-time priority changes go through
  the same Linux-compatible validation path.
- Narrowed `same_owner` in the shared scheduler permission helper to compare the
  caller effective UID against the target real/effective UID.
- Re-enabled `sched_setparam05` in `testcases/all.txt`.

## Validation

```bash
SSH_PORT=46046 NGINX_PORT=52104 REDIS_PORT=50403 IPERF_PORT=49225 \
LMBENCH_TCP_LAT_PORT=45474 LMBENCH_TCP_BW_PORT=45476 MEMCACHED_PORT=45451 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=sched_setparam05

SSH_PORT=46048 NGINX_PORT=52106 REDIS_PORT=50405 IPERF_PORT=49227 \
LMBENCH_TCP_LAT_PORT=45494 LMBENCH_TCP_BW_PORT=45496 MEMCACHED_PORT=45471 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=sched_setparam05

SSH_PORT=46049 NGINX_PORT=52107 REDIS_PORT=50406 IPERF_PORT=49228 \
LMBENCH_TCP_LAT_PORT=45504 LMBENCH_TCP_BW_PORT=45506 MEMCACHED_PORT=45481 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=sched_setparam05
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed behavior:

- before the fix, LTP reported successful `sched_setparam(..., 0)` calls where
  it expected `EPERM`
- after the fix, both the libc wrapper and raw syscall path returned `EPERM`
  for the unprivileged cross-thread priority changes checked by the testcase

## Impact / Residual Risk

- This fixes a real Linux-compatibility bug in scheduler permission handling.
- The change also aligns `sched_setparam()` with the existing
  `sched_setscheduler()` permission model, reducing the risk of the two syscalls
  diverging again.
- Remaining disabled Priority A scheduler work is now the affinity and
  `sched_{set,get}attr*` cases.
