# `rt_tgsigqueueinfo01`

## Goal

Re-enable `rt_tgsigqueueinfo01` in Phase 3 and verify that it passes on `/tmp`,
`/ext2`, and `/exfat`.

## Root Cause

This testcase was blocked by a real kernel compatibility gap. The guest kernel
did not expose an `rt_tgsigqueueinfo` syscall entry in the architecture
dispatch tables, so LTP could not exercise thread-directed queued realtime
signal delivery and treated the case as unsupported.

## Solution

- Added a new `rt_tgsigqueueinfo` syscall handler.
- Wired the handler into both the x86 and generic syscall dispatch tables.
- Implemented the Linux-compatible behavior needed by LTP: reading user
  `siginfo_t`, overriding `si_signo` with the requested signal number,
  rejecting invalid `si_code` values with `EPERM` for cross-process delivery,
  and routing the request through `tgkill` for exact thread targeting.
- Re-enabled `rt_tgsigqueueinfo01` in `testcases/all.txt`.

## Validation

```bash
SSH_PORT=45022 NGINX_PORT=51080 REDIS_PORT=49379 IPERF_PORT=48201 \
LMBENCH_TCP_LAT_PORT=44234 LMBENCH_TCP_BW_PORT=44236 MEMCACHED_PORT=44211 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=rt_tgsigqueueinfo01

SSH_PORT=45022 NGINX_PORT=51080 REDIS_PORT=49379 IPERF_PORT=48201 \
LMBENCH_TCP_LAT_PORT=44234 LMBENCH_TCP_BW_PORT=44236 MEMCACHED_PORT=44211 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=rt_tgsigqueueinfo01

SSH_PORT=45022 NGINX_PORT=51080 REDIS_PORT=49379 IPERF_PORT=48201 \
LMBENCH_TCP_LAT_PORT=44234 LMBENCH_TCP_BW_PORT=44236 MEMCACHED_PORT=44211 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=rt_tgsigqueueinfo01
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed behavior:

- self-thread queued signal delivery succeeded
- parent-to-thread queued signal delivery succeeded
- thread-to-thread queued signal delivery succeeded

## Impact / Residual Risk

- This closes the thread-directed queued realtime signal syscall gap and
  unlocks active LTP coverage for `rt_tgsigqueueinfo`.
- The implementation is intentionally scoped to the Linux semantics exercised
  by current LTP coverage; if later tests probe deeper permission or
  `siginfo_t` provenance details, follow-up refinement may still be needed.
