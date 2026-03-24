# Task 2.1: Family-Based Re-enable Triage Sheet

## Goal

Group disabled and filesystem-blocked LTP cases by subsystem family so future
re-enable work can proceed in coherent 5-to-10-case batches instead of
alphabetical one-offs.

## Problem Cause

The repository only had the raw enable list in `testcases/all.txt` plus
filesystem-specific blocklists. Those files are optimized for packaging, not
for planning. Contributors had to rediscover which disabled cases belonged to
the same subsystem before starting a batch.

## Solution

Added [`FAMILY_REENABLE_TRIAGE.md`](../FAMILY_REENABLE_TRIAGE.md), which:

- Re-groups the backlog using the family buckets defined in `TASKS.md`.
- Combines data from `all.txt`, `blocked/ext2.txt`, and `blocked/exfat.txt`.
- Records per-family counts for disabled and filesystem-blocked cases.
- Lists current filesystem-specific exclusions by family.
- Suggests initial 5-to-10-case batches contributors can use as the next
  debugging unit.
