# Task 1.3: Temporary Extra Blocklists

## Goal

Allow local LTP triage runs to inject temporary blocklists through
`EXTRA_BLOCKLISTS_DIRS` without editing the repository's persistent enable list
or filesystem blocklists.

## Problem Cause

Before this change, LTP packaging only knew about `testcases/all.txt` and the
built-in `/ext2` and `/exfat` exclusions. Developers had no low-friction way to
quarantine unstable cases locally, so ad hoc debugging either required editing
tracked files or running the whole batch.

## Solution

Extended [`Makefile`](../Makefile) so LTP packaging can consume a
whitespace-separated list of extra blocklist directories via
`EXTRA_BLOCKLISTS_DIRS`.

For each directory, packaging now applies:

- `all.txt` to every run target.
- `ext2.txt` only for `/ext2`.
- `exfat.txt` only for `/exfat`.

Entries can be absolute paths or paths relative to the LTP source directory.
The extra blocklists are layered on top of the built-in filters before the
final `runtest/syscalls` file is generated.
