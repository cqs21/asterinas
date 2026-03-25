# link08

## Goal

Enable `link08` on `/tmp`.

## Failure

`link08` checks several hard-link error cases, including creating a hard link on a read-only
mount. On `/tmp`, the read-only-mount subcase unexpectedly succeeded instead of returning `EROFS`.

## Root Cause

The VFS path mutation entry points only enforced permission checks on directory inodes. They did
not reject mutations on mounts or superblocks marked read-only, so `link()` could still create a
new directory entry after the mount had been remounted `MS_RDONLY`.

## Fix

- Added a shared read-only mount check in `Path`.
- Reused that check before creating new filesystem children, creating `O_TMPFILE` inodes, and
  before all directory mutation helpers reached `mknod()`, `link()`, `unlink()`, `rmdir()`, and
  `rename()`.
- Enabled `link08` in `testcases/all.txt`.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=link08`
