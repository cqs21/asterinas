# `statx05`

## Goal

Evaluate whether `statx05` can be re-enabled for Phase 3 and pass on `/tmp`,
then continue to `/ext2` and `/exfat` if the testcase is generically
applicable.

## Root Cause

`statx05` is not a generic `/tmp` coverage testcase. Upstream LTP defines it as
an `ext4`-only test that:

- formats a fresh device with `mkfs.ext4 -O encrypt`
- requires `e4crypt`
- verifies `STATX_ATTR_ENCRYPTED`

In the Asterinas guest, the run stops immediately with `TCONF` because
`mkfs.ext4` is not present in `$PATH`. The guest also does not expose `ext4` in
`/proc/filesystems`, so adding the user-space tool alone would still not make
the testcase runnable.

## Solution

- Kept `statx05` disabled in `testcases/all.txt`.
- Recorded the case as an environment and feature-gap item rather than a
  `/tmp` `statx()` regression.

## Validation

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=statx05
```

Observed result:

- `/tmp`: `CONF` with `Couldn't find 'mkfs.ext4' in $PATH`

`/ext2` and `/exfat` were not run because the testcase is explicitly `ext4`
only and already exits in setup before reaching any generic `/tmp`-vs-filesystem
behavior.

## Impact / Residual Risk

- Leaving `statx05` disabled does not hide a known `/tmp` compatibility bug.
- Re-enabling it would require a larger enablement effort: guest `ext4`
  support, `mkfs.ext4`, and `e4crypt`, plus any kernel support needed for the
  encrypted attribute path.
