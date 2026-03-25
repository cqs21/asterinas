# LTP Debugging and Development Tasks

This document tracks the engineering work needed to expand Asterinas'
effective LTP coverage.

## Scope

The current LTP setup does not treat
[`testcases/all.txt`](./testcases/all.txt) as a failure report.
Instead, it is the enable list that is packaged into initramfs. During
packaging, commented lines are removed and only enabled cases are copied
into `runtest/syscalls`; see [`Makefile`](./Makefile).

As of this document:

- `all.txt` contains a relatively small enabled subset of the upstream LTP
  syscalls suite.
- Many cases are still disabled by comments in `all.txt`.
- Additional filesystem-specific exclusions exist in
  [`testcases/blocked/ext2.txt`](./testcases/blocked/ext2.txt) and
  [`testcases/blocked/exfat.txt`](./testcases/blocked/exfat.txt).
- A recent `/tmp` batch run in the local workspace reported
  `Total Failures: 0`, so the main backlog is not only "fix active test
  failures" but also "systematically re-enable disabled coverage".

The work therefore has two goals:

1. Keep the currently enabled LTP subset stable.
2. Gradually convert disabled or blocked cases into passing coverage.

## Current Execution Model

Relevant entry points:

- [`../../run_syscall_test.sh`](../run_syscall_test.sh)
- [`run_ltp_test.sh`](./run_ltp_test.sh)
- [`Makefile`](./Makefile)
- [`testcases/all.txt`](./testcases/all.txt)

Current behavior:

- Top-level `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp`
  passes `SYSCALL_TEST_WORKDIR` and `EXTRA_BLOCKLISTS_DIRS` through the
  kernel command line.
- LTP packaging filters `all.txt`, then applies filesystem-specific
  blocklists for `/ext2` and `/exfat`.
- The current LTP runner executes `runltp -f syscalls` as one batch and
  only checks whether the final summary contains `Total Failures: 0`.
- Unlike the gVisor syscall runner, the LTP path does not yet support
  temporary extra blocklists, per-case failure summaries, or easy single
  test selection.

## Operating Principles

- Debug in small batches. Re-enable 5 to 10 cases from the same syscall
  family at a time.
- Keep `/tmp`, `/ext2`, and `/exfat` as separate validation targets.
- Record the failure class before changing code.
- Prefer fixing one kernel semantic issue that unlocks a family of cases
  over chasing isolated tests one by one.
- Do not remove blocklists permanently until the corresponding cases pass
  reliably in CI-oriented run modes.

## Failure Taxonomy

Each newly enabled or newly failing case should be classified into one of
these buckets:

- Missing syscall, flag, or option support.
- Wrong Linux-compatible errno or return semantics.
- VFS or filesystem semantic mismatch.
- Credential, permission, namespace, or capability semantic mismatch.
- Signal, timer, scheduler, futex, or wakeup ordering issue.
- Test harness limitation, packaging problem, or environment assumption.

This classification should be attached to every task batch so that the
backlog stays actionable.

## Standard Reproduction Flow

Use this order for every batch:

1. Run the batch on `/tmp`.
2. If `/tmp` passes, run the same batch on `/ext2`.
3. If `/ext2` passes or the remaining failures are known filesystem
   differences, run the same batch on `/exfat`.
4. Update the enable list or filesystem blocklists only after the results
   are understood.

Recommended commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp SYSCALL_TEST_WORKDIR=/ext2
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp SYSCALL_TEST_WORKDIR=/exfat
```

When a batch is being debugged, prefer reducing the test set before adding
instrumentation to the kernel.

## Phase 1: Improve the LTP Debugging Harness

These tasks should be completed first because they lower the cost of all
later kernel work.

### Task 1.1: Add single-case and batch selection support

Goal:

- Run one LTP case or a small explicit list without editing `all.txt`.

Suggested implementation:

- Support an environment variable such as `LTP_CASES=fcntl07,fcntl08`.
- Or support `LTP_CASE_FILE=/path/to/cases.txt`.
- Filter the packaged `runtest/syscalls` file against that selection before
  invoking `runltp`.

Done when:

- A developer can run a single case repeatedly without changing tracked
  files in `testcases/`.

### Task 1.2: Add per-case logs and a failure summary

Goal:

- Make the runner print a concise failed-case list in addition to the raw
  LTP log.

Suggested implementation:

- Parse the LTP log after the run and extract failed, broken, and skipped
  cases.
- Emit a machine-readable summary file under `SYSCALL_TEST_WORKDIR`.
- Keep the raw `result.log` for full inspection.

Done when:

- A failing run tells the developer exactly which cases failed without
  manual log scraping.

### Task 1.3: Support temporary extra blocklists for LTP

Goal:

- Match the gVisor flow, where temporary blocklists can be injected through
  `EXTRA_BLOCKLISTS_DIRS`.

Suggested implementation:

- Extend the LTP packaging or runner path to apply extra blocklists on top
  of `all.txt` and the built-in filesystem blocklists.
- Keep the persistent blocklists for stable known exclusions and use extra
  blocklists for local triage.

Done when:

- A developer can quarantine unstable cases locally without editing the
  repository blocklists.

### Task 1.4: Add stronger cleanup and run isolation

Goal:

- Eliminate false failures caused by leftover files or directories.

Suggested implementation:

- Clean `SYSCALL_TEST_WORKDIR` before and after each selected case or batch.
- Preserve logs in a separate path so cleanup does not remove diagnostics.

Done when:

- Re-running the same batch produces stable results without manual cleanup.

## Phase 2: Build a Structured Re-enable Workflow

### Task 2.1: Create family-based re-enable batches

Create a triage sheet or appendix that groups disabled and blocked cases by
subsystem rather than alphabetically.

Initial family buckets:

- File descriptor and eventing:
  `fcntl`, `epoll`, `dup*`, `eventfd*`, `pipe*`, `poll`, `select`.
- Filesystem and VFS:
  `open*`, `stat*`, `rename*`, `link*`, `unlink*`, `chmod*`, `chown*`,
  `sendfile*`, `xattr*`, `getcwd*`, `mkdir*`, `mknod*`.
- Process and scheduling:
  `clone*`, `fork*`, `wait*`, `sched*`, `pidfd*`, `ioprio*`.
- Signals and timers:
  `rt_sig*`, `signal*`, `signalfd*`, `sigaltstack*`, `clock_*`,
  `timer*`, `alarm*`.
- Memory and mapping:
  `mmap*`, `mprotect*`, `mremap*`, `msync*`, `munmap*`, `madvise*`,
  `memfd_create`.
- Credentials and security:
  `capget/capset`, `set*uid`, `set*gid`, `prctl`, `unshare`, `setns`.
- Networking and sockets:
  `socket*`, `bind*`, `connect*`, `accept*`, `send*`, `recv*`,
  `getsockopt/setsockopt`.
- Large missing feature areas:
  `ioctl`, `inotify`, `fanotify`, new mount API, `landlock`, `bpf`,
  `keyctl`, SysV IPC, `quotactl`.

Done when:

- Newly enabled work is always planned as a family batch, not as unrelated
  single tests.

### Task 2.2: Track each batch with a fixed template

For every batch, record:

- Enabled cases under test.
- Current run target: `/tmp`, `/ext2`, or `/exfat`.
- Failure taxonomy for each remaining failure.
- Suspected kernel subsystem.
- Owner.
- Status: `todo`, `debugging`, `blocked`, `ready-to-enable`, or `done`.

Done when:

- The backlog can be split among contributors without repeated rediscovery.

## Phase 3: Kernel Work Priorities

The order below is chosen by expected leverage, not by alphabetical order.

### Priority A: Extend partially supported high-yield families

Start here:

- `fcntl`
- `open/openat`
- `rename/link/statx`
- `sched*`
- `clock_*`
- `futex`
- `signal/signalfd`
- `epoll`

Why:

- These areas already have some passing coverage.
- A single semantic fix often unlocks multiple LTP cases.
- Several of these families are central to Linux compatibility and regress
  easily.

### Priority B: Reduce filesystem-specific blocklists

Current direction:

- Investigate why `/ext2` and `/exfat` need separate exclusions rather than
  sharing the `/tmp` behavior.
- Distinguish VFS bugs from concrete filesystem limitations.

Focus areas:

- Metadata updates and timestamps.
- rename/link/unlink semantics.
- append, truncate, fsync, and sendfile behavior.
- permission and ownership edge cases.
- symlink and path resolution corner cases.

Done when:

- Filesystem-specific blocklists shrink steadily instead of being treated as
  permanent debt.

### Priority C: Plan large missing feature areas as separate projects

Do not mix these into small bug-fix batches:

- `ioctl`
- `inotify` and `fanotify`
- new mount API: `fsopen`, `fsconfig`, `fsmount`, `fspick`
- `landlock`
- `bpf`
- `keyctl`
- SysV IPC families
- `quotactl`

Why:

- These are feature projects, not compatibility nits.
- They often need design work, permission-model decisions, and new kernel
  abstractions.

Done when:

- Each area has its own scoped design or implementation task list.

## Completion Criteria for Each Batch

A batch is complete only when all of the following hold:

1. `/tmp` passes for the selected cases.
2. `/ext2` and `/exfat` have been evaluated for the same cases.
3. Cases that now pass are enabled in [`testcases/all.txt`](./testcases/all.txt).
4. Cases that still fail only on a specific filesystem are documented in the
   corresponding filesystem blocklist with a short reason where needed.
5. Kernel fixes include regression coverage where practical.
6. No unrelated cases regress in the same family.

## Immediate Next Tasks

The recommended first iteration is:

1. Implement LTP single-case or case-file selection.
2. Implement LTP failure summary output.
3. Implement `EXTRA_BLOCKLISTS_DIRS` support for LTP.
4. Start the first family batch with either `fcntl` or `clock_*`.
5. Validate the same batch on `/tmp`, `/ext2`, and `/exfat`.

## Notes for Contributors

- Keep `unsafe` out of `kernel/`; any required unsafe work must stay within
  `ostd/`.
- Prefer fixing Linux semantics at the boundary rather than teaching tests
  about Asterinas-specific behavior.
- If a failure is caused by an intentional unsupported feature, make that
  explicit in the task record instead of leaving it as an unexplained block.
