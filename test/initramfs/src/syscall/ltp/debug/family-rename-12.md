# rename12

## Goal

Enable `rename12` on `/tmp`.

## Failure

`rename12` creates a sticky directory, switches to `nobody`, and then tries to rename a file
owned by another user. Linux should reject that rename with `EPERM` or `EACCES`, but Asterinas
incorrectly allowed it on `/tmp`.

## Root Cause

The VFS dentry layer did not enforce sticky-directory ownership rules for delete-like operations.
`rename()` could remove or replace entries in a sticky directory even when the caller owned
neither the directory nor the victim inode and lacked `CAP_FOWNER`.

## Fix

- Added sticky-directory permission checks in `DirDentry` for `rename`, `unlink`, and `rmdir`.
- Kept the checks aligned with Linux ownership rules based on `fsuid`, directory owner, victim
  owner, and `CAP_FOWNER`.
- Enabled `rename12` in `testcases/all.txt`.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=rename12`
