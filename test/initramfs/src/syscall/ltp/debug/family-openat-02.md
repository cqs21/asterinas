# openat02

## Goal

Enable `openat02` and confirm the `/tmp` coverage for `O_APPEND`, `O_CLOEXEC`, `O_LARGEFILE`,
`O_NOATIME`, `O_NOFOLLOW`, and `O_TRUNC`.

## Failure

No failure remained when this testcase was revalidated. `openat02` passed on `/tmp` with the
current kernel state.

## Root Cause

The earlier `open*` fixes had already covered the behaviors that `openat02` exercises:

- `O_NOATIME` handling on `tmpfs`
- `O_CLOEXEC` helper binary packaging
- `O_NOFOLLOW`/`O_TRUNC`/`O_APPEND` open semantics

Because of that, `openat02` no longer needed an additional fix.

## Fix

- No kernel code change was required for this testcase.
- Enable `openat02` in the LTP allowlist after validation.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=openat02`
