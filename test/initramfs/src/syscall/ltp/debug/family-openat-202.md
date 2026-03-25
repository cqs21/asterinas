# openat202

## Goal

Enable `openat202` on `/tmp` and verify the negative `openat2(2)` resolve-flag semantics.

## Failure

Before `openat2(2)` support was added, the testcase was skipped with `TCONF` because the syscall
was missing.

## Root Cause

`openat202` depends entirely on `openat2(2)` resolve handling. Without syscall support, LTP could
not verify the expected `EXDEV`, `ELOOP`, and `ENOENT` results for mount crossing, magic links,
symlinks, and scoped-root lookup.

## Fix

- Reused the new `openat2` path-resolution flow introduced for `openat201`.
- Verified that `/proc` mount crossings are rejected under `RESOLVE_NO_XDEV` and
  `RESOLVE_BENEATH`.
- Verified that procfs magic links are rejected under `RESOLVE_NO_MAGICLINKS`, regular symlinks
  are rejected under `RESOLVE_NO_SYMLINKS`, and absolute lookups are scoped by
  `RESOLVE_IN_ROOT`.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=openat202`
