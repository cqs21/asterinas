# Task 1.4: Cleanup and Run Isolation

## Goal

Prevent leftover files and directories in `SYSCALL_TEST_WORKDIR` from causing
false LTP failures across repeated reruns.

## Problem Cause

The runner previously used the syscall work directory both as scratch space and
as the place where diagnostics lived. That meant stale test artifacts could
survive across reruns unless the developer cleaned the directory manually.

## Solution

Updated [`run_ltp_test.sh`](../run_ltp_test.sh) to separate scratch data from
diagnostic artifacts:

- Batch cleanup now runs before and after each `runltp` invocation.
- Cleanup removes everything in `SYSCALL_TEST_WORKDIR` except `.ltp_logs`.
- Per-run diagnostics are written to `.ltp_logs/run-<timestamp>-<pid>/`.
- A stable `.ltp_logs/latest/` copy is refreshed after each run.

This keeps reruns isolated while preserving `result.log`, the summary files,
and per-case log fragments for later inspection.
