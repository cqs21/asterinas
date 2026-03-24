# `epoll_wait06`

## Goal

Enable `epoll_wait06` in Phase 3 and make it pass on `/tmp`.
This testcase verifies EPOLLET behavior on non-blocking pipes after the pipe
is intentionally resized with `fcntl(F_SETPIPE_SZ, ...)`.

## Root cause

Two kernel gaps blocked this testcase:

1. `fcntl(F_SETPIPE_SZ)` was not implemented, so testcase setup failed with
   `EINVAL`.
2. `pipe2(O_NONBLOCK)` did not propagate `O_NONBLOCK` into pipe file status
   flags. After `F_SETPIPE_SZ` started working, the second `write()` on a full
   pipe blocked instead of returning `EAGAIN`, causing LTP timeout.

## Solution

- Implement `F_SETPIPE_SZ` in `sys_fcntl` and wire it to pipe handles.
- Add pipe-capacity resizing support for empty pipes.
- Parse `pipe2` flags correctly and pass pipe status flags into anonymous pipe
  handles so `O_NONBLOCK` is honored.
- Re-enable `epoll_wait06` in `testcases/all.txt`.

## Validation

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=epoll_wait06
```

Observed result: `epoll_wait06` reported `PASS` on `/tmp` with all 9 checks
(`write`/`read` `EAGAIN`, EPOLLIN/EPOLLOUT edge-trigger expectations) passing.

## Impact / Residual risk

- This change unlocks both the testcase and real user-visible semantics for
  `pipe2(O_NONBLOCK)` and `F_SETPIPE_SZ`.
- Current pipe resizing supports empty-pipe updates, which is sufficient for
  this testcase; more complete Linux parity for resizing non-empty pipes can be
  handled in follow-up work if future LTP coverage requires it.
