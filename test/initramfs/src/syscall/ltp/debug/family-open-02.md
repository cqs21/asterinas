# `open02`

## Goal

Re-enable `open02` in Phase 3 and verify that it passes on `/tmp`, `/ext2`,
and `/exfat`.

## Root Cause

The kernel accepted `open(..., O_NOATIME)` from an unprivileged caller even
when Linux should reject it with `EPERM`.

This testcase checks two paths:

- opening a nonexistent file without `O_CREAT` -> `ENOENT`
- opening an existing file with `O_RDONLY | O_NOATIME` as an unprivileged user
  -> `EPERM`

The first path was already correct. The second failed because Asterinas parsed
`O_NOATIME` into `StatusFlags`, but the open path never enforced the Linux
permission gate for that flag.

## Solution

- Added an `O_NOATIME` permission check in `Path::open()` for existing files.
- Allowed `O_NOATIME` only when the caller effective UID matches the inode
  owner, or when the caller has `CAP_FOWNER`.
- Returned `EPERM` otherwise.
- Re-enabled `open02` in `testcases/all.txt`.

## Validation

```bash
SSH_PORT=46068 NGINX_PORT=52126 REDIS_PORT=50425 IPERF_PORT=49247 \
LMBENCH_TCP_LAT_PORT=45694 LMBENCH_TCP_BW_PORT=45696 MEMCACHED_PORT=45671 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=open02

SSH_PORT=46069 NGINX_PORT=52127 REDIS_PORT=50426 IPERF_PORT=49248 \
LMBENCH_TCP_LAT_PORT=45704 LMBENCH_TCP_BW_PORT=45706 MEMCACHED_PORT=45681 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=open02

SSH_PORT=46070 NGINX_PORT=52128 REDIS_PORT=50427 IPERF_PORT=49249 \
LMBENCH_TCP_LAT_PORT=45714 LMBENCH_TCP_BW_PORT=45716 MEMCACHED_PORT=45691 \
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=open02
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed behavior:

- before the fix, the unprivileged `O_NOATIME` subcheck unexpectedly succeeded
- after the fix, LTP observed the expected `EPERM` on all three workdirs

## Impact / Residual Risk

- This fixes a real open-path permission bug, not just testcase enablement.
- The check is intentionally narrow and only affects the Linux-specific
  `O_NOATIME` gate on existing files.
- Remaining Priority A work in the `open/openat` family should be checked next,
  starting from the next disabled testcase in `all.txt`.
