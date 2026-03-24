# Task 1.1: Single-Case and Batch Selection

## Goal

Allow developers to run one LTP case or a small explicit batch without
editing `testcases/all.txt`.

## Problem Cause

The original packaging path always turned the repository-wide enable list into
`runtest/syscalls` as a single batch. That forced local triage to mutate
tracked files even when a developer only wanted to retry one case.

## Solution

Extended the LTP packaging step in [`Makefile`](../Makefile) with two optional
selection inputs:

- `LTP_CASES`, a comma-separated case list.
- `LTP_CASE_FILE`, a file containing one case per line.

The Makefile now:

- Applies the existing filesystem blocklist filter first.
- Builds a normalized selection list from the optional inputs.
- Intersects that selection with the enabled and non-blocked cases.
- Fails early when the case file is missing, the selection is empty, or the
  selection has no surviving cases after filtering.

When no selection variables are set, packaging behavior stays unchanged.
