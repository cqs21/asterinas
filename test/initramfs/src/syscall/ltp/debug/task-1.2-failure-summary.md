# Task 1.2: Failure Summary and Per-Case Artifacts

## Goal

Make a failing LTP run immediately tell the developer which cases failed,
broke, or were skipped, while preserving raw logs for full inspection.

## Problem Cause

The original runner only printed `result.log` and checked whether the final
summary contained `Total Failures: 0`. Developers still had to scrape the log
manually to learn which cases were responsible.

## Solution

Extended [`run_ltp_test.sh`](../run_ltp_test.sh) so each run now produces:

- `result.log` with the raw LTP output.
- `ltp_summary.json` with machine-readable counts and case lists.
- `ltp_case_status.tsv` with per-case status lines.
- `ltp_failed_cases.txt`, `ltp_broken_cases.txt`, and
  `ltp_skipped_cases.txt`.
- `ltp_case_logs/` with per-case extracted log fragments.

The runner parses `TFAIL`, `TBROK`, `TSKIP`, and `TCONF` from the LTP log,
prints concise case buckets to the console, and keeps skipped-only runs
non-fatal while still failing the run for failed or broken cases.
