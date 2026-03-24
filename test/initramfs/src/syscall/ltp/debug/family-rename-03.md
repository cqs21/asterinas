# `rename03`

## Goal

Re-enable `rename03` in Phase 3 and verify that it passes on `/tmp`, `/ext2`,
and `/exfat`.

## Root Cause

There was no missing kernel behavior in this testcase. `rename03` exercises the
existing-target replacement path for:

- file to existing file
- empty directory to existing empty directory

The current rename implementation already preserves inode identity and removes
the old pathname as Linux does. The testcase was simply still disabled in the
packaged allowlist.

## Solution

- Re-ran `rename03` on all three validation targets.
- Confirmed the current VFS behavior already satisfies the testcase.
- Re-enabled `rename03` in `testcases/all.txt`.

## Validation

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=rename03

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=rename03

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=rename03
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

## Impact / Residual Risk

- This extends active rename-family coverage without any kernel code change.
- The remaining rename cases now focus on negative errno paths rather than the
  basic success path.
