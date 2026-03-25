# open14

## Goal

Enable `open14` and make `O_TMPFILE` work on `/tmp`.

## Failure

On `/tmp`, `open14` reported `TCONF: O_TMPFILE not supported`.

## Root Cause

The kernel did not recognize `O_TMPFILE`. Because the Linux `O_TMPFILE` bit pattern includes
`O_DIRECTORY`, the flag combination degraded into an ordinary writable open on the target
directory and failed with `EISDIR`, which LTP treated as "feature not supported".

There was also no way to represent an unnamed regular file that stays invisible in the directory
but can later be linked into the filesystem through `/proc/self/fd/<n>`.

## Fix

- Add `CreationFlags::O_TMPFILE`.
- Teach `Path::open()` to handle `O_TMPFILE` before normal directory-open validation.
- Add a VFS `create_tmpfile()` hook and implement it for `ramfs/tmpfs`.
- Represent unnamed tmpfiles as pseudo paths backed by detached inodes so:
  - the containing directory stays empty until `linkat()`
  - `/proc/self/fd/<n>` can still resolve back to the live inode and link it later

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=open14`
