# Task 2.2: Fixed Batch Tracker Template

## Goal

Provide a reusable tracking template so every LTP re-enable batch records the
same required fields: enabled cases, run target, failure taxonomy, suspected
subsystem, owner, and status.

## Problem Cause

`TASKS.md` defined the tracking requirements, but the repository had no
standard document contributors could copy. That made it easy for different
batches to omit fields or use inconsistent terminology, which slows parallel
triage.

## Solution

Added [`BATCH_TRACKER.md`](../BATCH_TRACKER.md) as the canonical template for
future LTP batches. The template includes:

- A fixed status vocabulary.
- An overview table for splitting work across contributors.
- A per-batch section that captures all required metadata.
- A failure-taxonomy table aligned with `TASKS.md`.
- A run-results matrix for `/tmp`, `/ext2`, and `/exfat`.

Also linked the template from [`TASKS.md`](../TASKS.md) so the workflow points
to the concrete artifact contributors should use.
