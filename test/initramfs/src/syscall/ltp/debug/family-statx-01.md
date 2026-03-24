# `statx01`

## Goal

Re-enable `statx01` in Phase 3 and verify that it passes on `/tmp`. Validate
the same fix on `/ext2` and `/exfat`, while treating filesystem-specific
limitations outside `/tmp` as documentation-only follow-up.

## Root Cause

The testcase cross-checks the `statx()` result against `/proc/self/mountinfo`.
Asterinas already returned a consistent `stx_mnt_id`, but
`/proc/self/mountinfo` still printed every mount's device tuple as `0:0`.
LTP compares the `statx().stx_dev_major:stx_dev_minor` tuple with the
corresponding `mountinfo` entry, so the test broke even though the mount ID was
correct.

## Solution

- Replaced the hard-coded `0:0` device numbers in procfs `mountinfo`.
- Derived each mount's device tuple from the mount root inode's
  `container_dev_id`.
- Decoded that device ID with `decode_device_numbers()` before formatting the
  `mountinfo` line.
- Re-enabled `statx01` in `testcases/all.txt`.

## Validation

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=statx01

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=statx01

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=statx01
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `TBROK` because `mknod()` returned `EINVAL` while creating the
  block-device fixture used by the testcase

## Impact / Residual Risk

- This fixes procfs `mountinfo` so its device tuple matches the inode metadata
  exported through `statx()`.
- The `/exfat` result is a filesystem capability gap around `mknod()`, not a
  regression in the `statx01` fix path.
