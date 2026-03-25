# link04

## Goal

Enable `link04` on `/tmp`.

## Failure

`link04` verifies that creating a hard link inside a directory without search/write permission is
rejected with `EACCES`. On `/tmp`, the testcase observed that `link()` succeeded where Linux
returns `EACCES`.

## Root Cause

The VFS mutation helpers delegated `link()`, `unlink()`, `rmdir()`, `rename()`, and `mknod()`
directly to the directory dentry implementation without first checking permission on the parent
directory inode. That skipped the required directory write/search permission gate and allowed
mutations through directories that should have been denied.

## Fix

- Added a shared directory-modification permission check in `Path`.
- Required `MAY_WRITE | MAY_EXEC` on the parent directory before `mknod()`, `link()`, `unlink()`,
  `rmdir()`, and `rename()`.
- Enabled `link04` in `testcases/all.txt`.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=link04`
