# `epoll_ctl04`

## Goal

Enable `epoll_ctl04` in the default LTP epoll batch by enforcing the Linux
limit on nested epoll instances and validating the result on `/tmp`, `/ext2`,
and `/exfat`.

## Root cause

`epoll_ctl04` builds a chain of nested epoll instances and expects the next
`EPOLL_CTL_ADD` to fail once the nesting depth reaches 5. Asterinas previously
accepted epoll-on-epoll additions without any topology check, so the testcase
incorrectly succeeded on the extra nesting step instead of returning
`ELOOP`/`EINVAL`.

## Solution

- Added nested-epoll validation in `EpollFile::add_interest()`.
- Reject additions that would create an epoll cycle.
- Reject additions when the target epoll subtree is already at the Linux
  nesting limit, returning `ELOOP`.
- Re-enable `epoll_ctl04` in `testcases/all.txt`.

## Validation

The testcase now passes on all three LTP workdirs:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=epoll_ctl04
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 LTP_CASES=epoll_ctl04
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat LTP_CASES=epoll_ctl04
```

Observed result after the fix:

- `epoll_ctl(..., EPOLL_CTL_ADD, ...) with number of nesting is 5 : ELOOP (40)`

## Impact / Residual risk

- The new check matches the tested Linux behavior for nested epoll graphs and
  also closes the obvious epoll-cycle hole.
- The implementation walks the epoll interest graph when adding nested epolls,
  so any future changes to epoll graph semantics should keep the traversal and
  lock behavior under review.
