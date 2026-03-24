# `statx06`

## Goal

Evaluate whether `statx06` can be re-enabled for Phase 3 and pass on `/tmp`,
then continue to `/ext2` and `/exfat` if the testcase targets generic
`statx()` semantics.

## Root Cause

`statx06` is also not a generic `/tmp` testcase. Upstream LTP marks it as an
`ext4` device test that reformats a block device and checks timestamp fields on
that mounted filesystem.

The Asterinas guest reaches the testcase, selects the scratch disk as
`LTP_DEV=/dev/vdc`, and then exits with `TCONF` because `mkfs.ext4` is missing
from `$PATH`. As with `statx05`, the guest does not currently advertise `ext4`
support in `/proc/filesystems`, so this is not a `/tmp` syscall regression that
can be fixed by adjusting `statx()` alone.

## Solution

- Kept `statx06` disabled in `testcases/all.txt`.
- Classified the testcase as blocked on guest `ext4` runtime support instead of
  a bug in the current `/tmp` `statx()` implementation.

## Validation

```bash
make clean
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=statx06
```

Observed result:

- `/tmp`: `CONF` with `Couldn't find 'mkfs.ext4' in $PATH`

`/ext2` and `/exfat` were not run because the testcase is explicitly tied to an
`ext4` scratch-device workflow and already fails during environment setup.

## Impact / Residual Risk

- The current `statx06` exclusion is an environment and feature-gap decision,
  not a known failure of `/tmp`.
- If `ext4` support is added later, this testcase should be revisited because
  it exercises timestamp-update semantics on a mounted `ext4` filesystem.
