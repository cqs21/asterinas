# openat03

## Goal

Enable `openat03` and confirm the `openat(..., O_TMPFILE, ...)` workflow passes on `/tmp`.

## Failure

No separate failure remained after revalidation. `openat03` passed on `/tmp` once the earlier
`O_TMPFILE` support was in place.

## Root Cause

`openat03` exercises the same core behavior as `open14`, but through `openat()`:

- create unnamed tmpfiles in directories
- keep directories empty until explicit linking
- link them back through `/proc/self/fd/<n>`
- preserve data and file mode semantics

Those requirements were already satisfied by the `open14` fix that added `O_TMPFILE` handling for
`tmpfs/ramfs`.

## Fix

- No additional kernel code change was required for this testcase.
- Enable `openat03` in the LTP allowlist after validation.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=openat03`
