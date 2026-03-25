# open06

## Goal

Enable `open06` and verify its FIFO `ENXIO` behavior on supported filesystems.

## Result

No kernel change was needed. The testcase already passed on `/tmp` and `/ext2`.

## Root Cause

`open06` had remained disabled even though the kernel already returned `ENXIO` for
`open(O_NONBLOCK | O_WRONLY)` on a FIFO with no reader.

## Filesystem Notes

`/exfat` does not support FIFO creation in the current setup. The testcase breaks earlier at
`mkfifo("tmpfile", 0644)` with `EINVAL`, so this is a filesystem capability gap rather than an
`open()` bug.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=open06`
- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp SYSCALL_TEST_WORKDIR=/ext2 LTP_CASES=open06`
- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp SYSCALL_TEST_WORKDIR=/exfat LTP_CASES=open06`
