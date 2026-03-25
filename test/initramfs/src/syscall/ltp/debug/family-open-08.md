# open08

## Goal

Enable `open08` and verify the expected `open()` error paths for the main filesystems used by the
LTP run.

## Result

No kernel change was needed. The testcase passed on `/tmp` and `/ext2`.

## Coverage

`open08` validated these error paths successfully on `/tmp` and `/ext2`:

- `EEXIST` for `O_CREAT | O_EXCL`
- `EISDIR` for write access on a directory
- `ENOTDIR` for `O_DIRECTORY` on a non-directory
- `ENAMETOOLONG` for an oversized pathname
- `EACCES` for a `0600` file opened by `nobody`
- `EFAULT` for an invalid pathname pointer

## Filesystem Notes

`/exfat` failed only on the permission case. After switching to `nobody`, opening the `0600`
`user2_0600` file with `O_WRONLY` still succeeded. This indicates missing Unix permission
enforcement on exfat rather than an `open()` regression in the generic VFS path.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=open08`
- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp SYSCALL_TEST_WORKDIR=/ext2 LTP_CASES=open08`
- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp SYSCALL_TEST_WORKDIR=/exfat LTP_CASES=open08`
