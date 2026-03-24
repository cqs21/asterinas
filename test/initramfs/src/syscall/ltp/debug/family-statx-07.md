# `statx07`

## Goal

Evaluate whether `statx07` can be re-enabled for Phase 3 and pass on `/tmp`,
then continue to `/ext2` and `/exfat` if it is a generic `statx()` testcase.

## Root Cause

`statx07` is an NFS-specific testcase. Upstream LTP sets `.filesystems =
{"nfs"}` and requires the `exportfs` command to create a local NFS
server/client setup before checking `AT_STATX_FORCE_SYNC` and
`AT_STATX_DONT_SYNC`.

In the Asterinas guest, the testcase exits with `TCONF` immediately because
`exportfs` is not present in `$PATH`. The guest also does not advertise `nfs` in
`/proc/filesystems`, so this is outside the current `/tmp` compatibility scope.

## Solution

- Kept `statx07` disabled in `testcases/all.txt`.
- Recorded the testcase as blocked on missing NFS environment and filesystem
  support rather than a bug in core `statx()` behavior on `/tmp`.

## Validation

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=statx07
```

Observed result:

- `/tmp`: `CONF` with `Couldn't find 'exportfs' in $PATH`

`/ext2` and `/exfat` were not run because the testcase only targets an NFS
mount workflow and never reaches generic local-filesystem behavior.

## Impact / Residual Risk

- Leaving `statx07` disabled does not block the `/tmp`-first goal for Priority
  A coverage.
- Re-enabling it would require an NFS-capable guest environment in addition to
  any kernel work needed for remote `statx()` cache-synchronization semantics.
