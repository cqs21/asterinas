# sigaction01

## Goal

Enable `sigaction01` on `/tmp`.

## Result

`sigaction01` now passes on `/tmp`. The testcase verifies that `SA_RESETHAND` does not clear
`SA_SIGINFO` while the handler is running, that the delivered signal is masked unless
`SA_NODEFER` is set, and that the original handler mask still applies.

## Root Cause

The kernel reset one-shot signal dispositions too early during signal dequeue. That made an
in-handler `sigaction()` query observe the default disposition instead of the installed user
handler, so `SA_SIGINFO` appeared to be cleared before `rt_sigreturn`.

## Fix

- Delayed `SA_RESETHAND` disposition reset until `rt_sigreturn`.
- Stored the installed one-shot `SigAction` in thread-local state while entering the handler.
- Reset the disposition on return only if the current action still matches the originally
  installed handler, so handler-side `sigaction()` updates are preserved.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=sigaction01`
