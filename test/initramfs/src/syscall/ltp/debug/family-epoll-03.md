# `epoll_wait05`

## Goal

Enable `epoll_wait05` in the default LTP epoll batch by fixing the `/tmp`
failure first, then verifying the same testcase on `/ext2` and `/exfat`.

## Root cause

`epoll_wait05` creates a TCP listener with `bind(INADDR_ANY)` and then connects
to it through `127.0.0.1`. Asterinas rejected `0.0.0.0` in
`get_iface_to_bind()`, because the bind path only accepted exact matches
against concrete local IPv4 addresses. That made `bind()` fail with
`EADDRNOTAVAIL` before the testcase could reach the `EPOLLRDHUP` check.

## Solution

- Accept `Ipv4Address::UNSPECIFIED` in `get_iface_to_bind()` instead of
  treating it as a missing local address.
- Map the wildcard bind to `loopback_iface()` for now, which preserves the
  localhost listener that `epoll_wait05` needs.
- Re-enable `epoll_wait05` in `testcases/all.txt`.

## Validation

The testcase now passes on all three LTP workdirs:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=epoll_wait05
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 LTP_CASES=epoll_wait05
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat LTP_CASES=epoll_wait05
```

Observed behavior after the fix:

- `bind(INADDR_ANY)` succeeds.
- The listener accepts the local connection through `127.0.0.1`.
- `epoll_wait()` reports `EPOLLRDHUP` as expected.

## Impact / Residual risk

- This is a targeted compatibility fix for localhost wildcard binds; it is not
  a full Linux-style `INADDR_ANY` implementation across all interfaces.
- Because sockets are still bound to one iface internally, wildcard listeners
  that should be reachable through non-loopback addresses may need a broader
  design change later.
