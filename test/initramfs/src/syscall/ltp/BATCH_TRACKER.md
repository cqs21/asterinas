# LTP Batch Tracker Template

This document provides a fixed template to track LTP re-enable batches.
Create one section per batch and keep status updated in the overview table.

## Status Values

- `todo`: Defined but not started.
- `debugging`: Reproduction and kernel or harness debugging in progress.
- `blocked`: Waiting on prerequisite support, design decision, or external dependency.
- `ready-to-enable`: Batch passes target runs and is ready to update enable or block lists.
- `done`: Enable or blocklist updates merged and validated.

## Batch Overview

| Batch ID | Family | Owner | Status | Current run target | Suspected subsystem |
|----------|--------|-------|--------|--------------------|---------------------|
| example-fcntl-01 | File descriptor and eventing | @owner | todo | /tmp | `kernel/src/fs`, `kernel/src/syscall` |

## Per-Batch Template

Copy this section for each new batch.

### Batch: `<batch-id>`

- Family: `<family-name>`
- Owner: `<github-id-or-name>`
- Status: `todo | debugging | blocked | ready-to-enable | done`
- Current run target: `/tmp | /ext2 | /exfat`
- Enabled cases under test:
  - `<case-name-1>`
  - `<case-name-2>`
  - `<case-name-3>`
- Suspected kernel subsystem:
  - `<crate-or-module-1>`
  - `<crate-or-module-2>`

#### Failure Taxonomy Mapping

| Case | Run target | Taxonomy class | Notes |
|------|------------|----------------|-------|
| `<case-name>` | `/tmp` | Missing syscall, flag, or option support | `<short-note>` |
| `<case-name>` | `/ext2` | VFS or filesystem semantic mismatch | `<short-note>` |

Taxonomy class must use one of:

- Missing syscall, flag, or option support.
- Wrong Linux-compatible errno or return semantics.
- VFS or filesystem semantic mismatch.
- Credential, permission, namespace, or capability semantic mismatch.
- Signal, timer, scheduler, futex, or wakeup ordering issue.
- Test harness limitation, packaging problem, or environment assumption.

#### Run Results

| Target | Command | Result | Log path |
|--------|---------|--------|----------|
| `/tmp` | `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp` | `<pass/fail>` | `<path>` |
| `/ext2` | `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp SYSCALL_TEST_WORKDIR=/ext2` | `<pass/fail>` | `<path>` |
| `/exfat` | `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp SYSCALL_TEST_WORKDIR=/exfat` | `<pass/fail>` | `<path>` |

#### Decision

- Enable-list changes:
  - `<update in testcases/all.txt>`
- Filesystem blocklist changes:
  - `<update in testcases/blocked/ext2.txt or exfat.txt>`
- Follow-up items:
  - `<next-step-1>`
  - `<next-step-2>`
