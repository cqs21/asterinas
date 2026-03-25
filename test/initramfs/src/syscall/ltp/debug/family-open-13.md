# open13

## Goal

Enable `open13` and make the `O_PATH` regression checks pass for `/tmp`.

## Failure

On `/tmp`, `open13` failed because operations that should be rejected on `O_PATH` file
descriptors still succeeded or returned the wrong errno:

- `fchmod()` succeeded
- `fchown()` succeeded
- `fgetxattr()` returned `EOPNOTSUPP` instead of `EBADF`

## Root Cause

The syscall layer already treated `O_PATH` specially for read, write, `ioctl`, and `mmap`, but
`fchmod()` and `fchown()` operated on the underlying inode path without checking whether the file
descriptor was opened with `O_PATH`.

For xattrs, `fgetxattr()` reused the path-based lookup flow. That flow parsed the xattr name and
namespace before checking whether the file descriptor itself was valid for data-bearing inode
operations, so an invalid xattr namespace escaped first as `EOPNOTSUPP`.

## Fix

- Reject `fchmod()` on `O_PATH` descriptors with `EBADF`.
- Reject `fchown()` on `O_PATH` descriptors with `EBADF`.
- Add a shared xattr file-handle accessibility check and run it before xattr name parsing so
  `fgetxattr()` on `O_PATH` also returns `EBADF`.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=open13`
