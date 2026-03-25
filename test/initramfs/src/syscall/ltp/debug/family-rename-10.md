# rename10

## Goal

Enable `rename10` on `/tmp`.

## Failure

`rename10` expects `rename()` to reject both an overlong destination path and an overlong
destination basename with `ENAMETOOLONG`. On `/tmp`, the long path case already failed correctly,
but renaming to a single overlong filename incorrectly succeeded.

## Root Cause

The VFS rename path only validated `"."` and `".."` for the source and destination names.
Component length checks were present in lookup and create paths, but `DirDentry::rename()` did not
enforce the filesystem `namelen` limit before dispatching to the inode `rename()` operation. On
`ramfs`, that let an overlong destination basename bypass the usual filename-length checks.

## Fix

- Added `namelen` validation to `DirDentry::rename()` for both the source and destination names.
- Enabled `rename10` in `testcases/all.txt`.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=rename10`
