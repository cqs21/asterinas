# diagnose-and-fix

A **diagnose-and-fix skill** for the Asterinas OS kernel.
Given either a code location (`file:lines`) or a failing conformance test, it works out
what Linux does, whether Asterinas matches, and why it diverges — then, when it's a
fixable **bug** and the mode allows, applies a **minimal** fix in an isolated git
worktree and emits a fix-only `patch.diff` plus a human-readable `root-cause.md`.

It is the third stage of the [`fix-conformance`](../fix-conformance/) workflow, but it
is **independently useful**: point it at a few lines of kernel code and it will tell you
whether they match Linux and, if you want, fix them.

It is **agent-agnostic** (Claude Code and Codex) and **local-first** (works in a
worktree branched from `main`; your current branch is never touched).

## Verdicts

Every run produces a **verdict**:

- `bug` — Asterinas has the implementation but diverges from Linux in a way a minimal,
  localized change can fix.
- `no-bug` — Asterinas already matches Linux; the failure is the test's own assumption,
  a harness/environment issue, or a flaky race.
- `missing-feature` — matching Linux needs a large new implementation, beyond a minimal
  fix.

Only `bug` is fixed automatically. A `no-bug`/`missing-feature` verdict is reported with
a `suggested_direction` and **not** fixed — unless you explicitly ask it to continue
along a direction (see below).

## Fix gating

Whether it edits code depends on the **mode**:

- **`report-only`** (default when you call it standalone with no explicit fix ask) —
  diagnose and report; make no edits. You decide what to do next.
- **`auto-fix`** (default when the workflow calls it) — fix a `bug` immediately.

Override for a **user-specified** test: pass a `continue_direction` and it will attempt
a fix even on a `no-bug`/`missing-feature` verdict, along the direction you give.

## Quick start

`diagnose-and-fix` is a skill, not a binary — trigger it from inside an agent session.

- **Claude Code:** `/diagnose-and-fix kernel/src/syscall/epoll.rs:120-155`,
  or ask: *"Use diagnose-and-fix to check whether `kernel/src/.../epoll.rs` lines
  120-155 match Linux `epoll_pwait2` semantics."*
- **Codex:** *"Use the diagnose-and-fix skill on the gvisor `epoll_test` failure and,
  if it's a bug, fix it."*

Inputs it accepts:

```
# A code range — the "give it a file and some lines" use case.
kernel/src/syscall/epoll.rs:120-155

# A conformance test + suite.
suite=gvisor test=epoll_test

# Ask it to fix along a direction even if it's not strictly a bug.
suite=gvisor test=epoll_test \
    continue_direction="switch the timeout to a monotonic-clock deadline"
```

The Linux reference is established **local-first** (a local kernel tree, man-pages, or
gVisor test source if present) and **otherwise fetched** (man7.org, torvalds/linux, or
the gVisor test source); the exact source is recorded in the output.

## Prerequisites

Applying a fix builds the kernel to check it compiles, which needs the Asterinas Docker
container (see [`AGENTS.md`](../../../AGENTS.md)). Pure diagnosis (`report-only` with no
build) works anywhere the source is readable. `kernel/` is safe Rust — the fix never
adds `unsafe` there.

## What you get back

- `diagnose.json` — structured verdict, Linux reference, located implementation,
  root cause, `suggested_direction`, and (if fixed) the changed files.
- `root-cause.md` — the human-readable analysis.
- `patch.diff` — the **fix-only** diff (no blocklist edit, no regression test), when a
  fix was applied. Standalone, `patch.diff` + `root-cause.md` is the complete
  deliverable.

## Design

The JSON schema, worktree/git rules, verdict gating, and shared vocabulary live in
[`../shared/conformance-workflow-contract.md`](../shared/conformance-workflow-contract.md)
(§2, §4, §5, §0). Read it first to understand or modify this skill.
