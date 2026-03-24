# `epoll_ctl05`

## Goal

Re-enable `epoll_ctl05` in Phase 3 and verify that the nested-epoll loop check
works consistently on `/tmp`, `/ext2`, and `/exfat`.

## Root cause

`epoll_ctl05` is a follow-up coverage case for nested epoll topologies. It
builds an epoll chain, removes the first edge, and then tries to add the tail
epoll back to the head. Linux must reject that final `EPOLL_CTL_ADD` with
`ELOOP` because it would recreate an epoll cycle.

After the earlier `epoll_ctl04` fix, Asterinas already had the required graph
walk and loop detection in `EpollFile::check_nested_epoll()`. So `epoll_ctl05`
was not blocked by a new kernel bug; it was simply still commented out in the
LTP testcase list and had not been revalidated end-to-end.

## Solution

- Re-run `epoll_ctl05` on `/tmp`, `/ext2`, and `/exfat`.
- Confirm that all three workdirs return `ELOOP` at the expected add step.
- Re-enable `epoll_ctl05` in `testcases/all.txt`.

## Validation

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=epoll_ctl05
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 LTP_CASES=epoll_ctl05
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat LTP_CASES=epoll_ctl05
```

Observed result on all three workdirs:

- `epoll_ctl(..., EPOLL_CTL_ADD, ...) : ELOOP (40)`

Each run finished with `PASS`.

## Impact / Residual risk

- No kernel code changes were needed for this testcase; the earlier nested
  epoll fix already covered the required behavior.
- This re-enable primarily increases confidence that both the depth-limit case
  (`epoll_ctl04`) and the explicit loop case (`epoll_ctl05`) stay covered by
  LTP.
