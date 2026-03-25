# open10

## Goal

Enable `open10` and make the `open()` create path honor the expected GID inheritance rules on the
main filesystems used by the LTP run.

## Failure

`open10` checks that newly created files inherit:

- the caller's GID in a normal directory
- the parent directory's GID in a directory with `S_ISGID`

On `/tmp`, every created file was owned by group `0`, so the testcase failed immediately on all
group ownership assertions.

## Root Cause

`tmpfs` is currently backed by `ramfs`, and the ramfs create path initialized every new inode with
`root:root` ownership. It ignored the current thread's `fsuid/fsgid`, and it also ignored the
parent directory's `S_ISGID` bit.

`ext2` already used the caller's `fsgid` for new inodes, but it still missed the parent-directory
`S_ISGID` inheritance rule, so files created inside a setgid directory would not inherit the parent
group.

## Fix

- Teach `ramfs` inode creation to initialize ownership from the current thread's
  `fsuid/fsgid`.
- When the parent directory has `S_ISGID`, inherit the parent `gid` for new children.
- Propagate `S_ISGID` onto newly created directories when required by the parent directory.
- Apply the same parent-`gid` inheritance rule in the `ext2` create path.

## Filesystem Notes

`/tmp` and `/ext2` now pass.

`/exfat` still fails before the actual inheritance checks because after `chown(dir_a, nobody,
free_gid)`, `dir_a` already reports `S_ISGID` set. That is a filesystem-specific metadata semantics
issue in exfat, not a remaining generic `open()` regression.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=open10`
- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp SYSCALL_TEST_WORKDIR=/ext2 LTP_CASES=open10`
- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp SYSCALL_TEST_WORKDIR=/exfat LTP_CASES=open10`
