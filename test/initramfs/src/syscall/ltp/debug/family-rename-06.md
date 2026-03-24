# `rename06`

## Goal

Re-enable `rename06` in Phase 3 and verify that it passes on `/tmp`, `/ext2`,
and `/exfat`.

## Root Cause

There was no remaining kernel defect in this case. `rename06` checks the
negative path where renaming a directory into its own subdirectory must fail
with `EINVAL`.

The current VFS path already returns Linux-compatible `EINVAL` on all tested
targets. The testcase had simply remained disabled in
`testcases/all.txt`.

## Solution

- Re-ran `rename06` on `/tmp`, `/ext2`, and `/exfat`.
- Confirmed that the existing implementation already returns the expected
  `EINVAL` errno.
- Re-enabled `rename06` in `testcases/all.txt`.

## Validation

```bash
SSH_PORT=42022 NGINX_PORT=48080 REDIS_PORT=46379 IPERF_PORT=45201 \
LMBENCH_TCP_LAT_PORT=41234 LMBENCH_TCP_BW_PORT=41236 MEMCACHED_PORT=41211 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=rename06

SSH_PORT=42022 NGINX_PORT=48080 REDIS_PORT=46379 IPERF_PORT=45201 \
LMBENCH_TCP_LAT_PORT=41234 LMBENCH_TCP_BW_PORT=41236 MEMCACHED_PORT=41211 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=rename06

SSH_PORT=42022 NGINX_PORT=48080 REDIS_PORT=46379 IPERF_PORT=45201 \
LMBENCH_TCP_LAT_PORT=41234 LMBENCH_TCP_BW_PORT=41236 MEMCACHED_PORT=41211 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=rename06
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed errno on all three runs:

- `rename()` rejected the operation with `EINVAL`

## Impact / Residual Risk

- This extends active rename-family negative-path coverage without a kernel
  code change.
- During validation, QEMU port auto-selection in `tools/qemu_args.sh` hit a
  duplicate host port once, so the verification commands above pin distinct
  host ports to avoid a false start unrelated to the testcase itself.
- The Priority A rename todo list is now empty.
