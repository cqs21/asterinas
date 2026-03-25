# open11

## Goal

Enable `open11` and make the generic `open()` success and error-path matrix pass on the primary
LTP filesystems.

## Failure

On `/tmp`, two cases incorrectly succeeded:

- opening an existing symlink-to-directory with `O_RDONLY | O_CREAT`
- opening an existing directory with `O_RDONLY | O_CREAT`

Both should fail with `EISDIR`.

## Root Cause

The resolved-path open flow rejected `O_CREAT | O_EXCL` on existing targets, but it did not apply
the Linux rule that `open()` with `O_CREAT` must fail with `EISDIR` when the resolved target is an
existing directory.

## Fix

Add the missing `EISDIR` check in the resolved `Path::open()` path when `O_CREAT` targets an
existing directory.

## Filesystem Notes

`/tmp` and `/ext2` now pass.

`/exfat` fails during testcase setup because `link(t_reg, t_link_reg)` returns `EINVAL`. `open11`
requires hard-link coverage, so this remains an exfat capability gap rather than a generic `open()`
issue.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=open11`
- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp SYSCALL_TEST_WORKDIR=/ext2 LTP_CASES=open11`
- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp SYSCALL_TEST_WORKDIR=/exfat LTP_CASES=open11`
