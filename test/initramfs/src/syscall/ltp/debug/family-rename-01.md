# `rename01`

## Goal

Re-enable `rename01` in Phase 3 and verify that it passes on `/tmp`, `/ext2`,
and `/exfat`.

## Root Cause

There was no kernel bug here. The testcase had simply remained commented out in
`testcases/all.txt` even though the current VFS rename path already handled the
covered semantics:

- renaming a file onto a non-existent path
- renaming a directory onto a non-existent path
- preserving inode identity across the rename
- making the old pathname disappear with `ENOENT`

## Solution

- Re-ran `rename01` on all three validation targets.
- Confirmed that the existing implementation already matches Linux for this
  coverage slice.
- Re-enabled `rename01` in `testcases/all.txt`.

## Validation

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=rename01

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=rename01

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=rename01
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

## Impact / Residual Risk

- This increases active rename-family coverage without changing kernel code.
- The remaining rename backlog now shifts to negative and replacement cases
  (`rename03` through `rename06`), which probe stricter errno semantics.
