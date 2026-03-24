# `clock_gettime02`

## Goal

Re-enable `clock_gettime02` in Phase 3 and verify that it passes on `/tmp`,
`/ext2`, and `/exfat`.

## Root Cause

`clock_gettime02` was not blocked by a clock syscall bug. The testcase calls
`tst_get_max_clocks()`, which asks LTP's `tst_kconfig` helper to parse a Linux
kernel `.config`. Asterinas does not expose one in the guest by default, so
the testcase stopped early with:

- `TBROK: Cannot parse kernel .config`

That made the failure a harness/environment mismatch rather than a kernel
semantic incompatibility.

## Solution

- Added a small synthetic `kernel.config` file to the packaged LTP payload.
- Updated the LTP runner to export `KCONFIG_PATH=/opt/ltp/kernel.config` when
  no explicit `KCONFIG_PATH` is already set.
- Kept the config conservative and explicitly marked
  `CONFIG_POSIX_AUX_CLOCKS` as not set, which makes LTP use the normal
  `MAX_CLOCKS` limit instead of auxiliary clock IDs.
- Re-ran `clock_gettime02` on `/tmp`, `/ext2`, and `/exfat`, then re-enabled
  the testcase in `testcases/all.txt`.

## Validation

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=clock_gettime02

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=clock_gettime02

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=clock_gettime02
```

Observed result on all three workdirs:

- `clock_gettime02` finished with `PASS`
- invalid clock IDs returned `EINVAL`
- valid clock IDs with a bad userspace pointer returned `EFAULT`

## Impact / Residual risk

- This change improves LTP compatibility for any testcase that depends on
  `tst_kconfig`.
- The packaged config is intentionally minimal. If future LTP cases depend on
  more kernel config probes, the compatibility file may need additional
  conservative entries.
