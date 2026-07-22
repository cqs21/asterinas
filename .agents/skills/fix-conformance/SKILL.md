---
name: fix-conformance
description: End-to-end conformance-fix workflow — select a not-yet-passing test, diagnose against Linux, fix, write a regression test, verify no regressions, and package a patch + PR description. Runs each stage in an isolated subagent with structured JSON hand-offs, persists state for resume, and loops N times (one independent patch per problem). Use to fix conformance failures start-to-finish; the individual stage skills can also be called on their own.
---

# fix-conformance (orchestrator)

Drive the full workflow: **select → diagnose-and-fix → regress-and-verify →
package-patch**, once per problem, for `N` problems. Each stage runs in its **own
spawned subagent** (clean context); stages hand off through **JSON files** on disk; a
per-problem `state.json` makes the run **resumable**; each problem gets its **own
worktree** and produces **one independent patch**.

Read `../shared/conformance-workflow-contract.md` in full first — it defines the
directory layout (§1), every JSON schema (§2), the subagent spawn model (§3), the
worktree/git rules (§4), the retry/completion rules (§5), and vocabulary (§0). This
skill only orchestrates; the real work lives in the four stage skills.

## Inputs

- `suite` (**required**) — `gvisor` | `ltp` | `kselftest` | `xfstests`.
- `work-dir` (**required**) — persistent directory for artifacts + state.
- `test` (optional) — a specific test → `source = "user-specified"`; else auto-pick.
- `count` / `N` (optional, default `1`) — number of problems to fix before stopping.
  The whole loop runs once per problem.
- `continue_direction` (optional) — for a user-specified test, a fix direction that
  authorizes fixing even a `no-bug`/`missing-feature` verdict.

## Setup / resume

1. Ensure `<work-dir>` exists. Read `run.json` if present (resuming): recover `suite`,
   `target_count`, `attempted_tests`, `completed_problems`. Otherwise create it.
2. For each existing problem sub-dir with a `state.json` whose `outcome` is null,
   **resume** it: skip `done` stages, re-enter at the first non-`done` stage using the
   JSON already on disk. Its worktree should still exist (§4 keeps it on interruption);
   if `state.json.worktree` is missing, re-create it from `main` and re-run from
   `diagnose-fix` (the code changes were lost with it).

## The per-problem loop

Repeat until `count` problems reach `outcome = "packaged"`, **or** a user-specified
test terminates the run, **or** auto-pick runs out of candidates:

### Stage A — select (subagent)
Spawn `select-conformance-test` with `suite`, `test` (first iteration only, if given),
and `exclude = attempted_tests`. It writes `select.json`. Derive `problem_id`
(`<suite>__<test>` sanitized, §1), create `<work-dir>/<problem-id>/`, copy `select.json`
in, and initialize `state.json` (`stages.select = "done"`, others `pending`,
`attempts: 0`, `outcome: null`). Append `test` to `attempted_tests` in `run.json`.

### Stage B — create the worktree
`git worktree add <repo>/.worktrees/<problem-id> -b conformance/<problem-id> main`.
Record the path in `state.json.worktree`.

### Stage C — diagnose-and-fix (subagent)
Set `state.diagnose-fix = "in-progress"`, `attempts += 1`. Spawn `diagnose-and-fix` in
`auto-fix` mode with `select.json`, the `worktree` path, `continue_direction` (if any),
and — on a retry — the prior `verify.json`. Read `diagnose.json` from disk.

- `verdict == "bug"`, or a user-specified non-`bug` with `continue_direction` → it
  should have applied a fix (`fix_applied: true`). Mark `diagnose-fix = "done"`,
  continue to Stage D.
- `verdict` is `no-bug`/`missing-feature` and no continue authorization:
  - **user-specified** test → set `outcome = "abandoned-<verdict>"`, tear down the
    worktree, **stop the whole run**, and report for the user's decision (include
    `suggested_direction`).
  - **auto-picked** test → set `outcome = "abandoned-<verdict>"`, tear down the
    worktree, and go pick the **next** candidate (Stage A) without consuming a success.

### Stage D — regress-and-verify (subagent)
Set `regress-verify = "in-progress"`. Spawn `regress-and-verify` with `select.json`,
`diagnose.json`, and the `worktree`. Read `verify.json`.

- `status == "ok"` → mark `regress-verify = "done"`, continue to Stage E.
- `status` is `selected-failed`/`regressed`:
  - if `attempts < 2` → loop back to **Stage C** (retry with `verify.json`).
  - else → set `outcome = "gave-up"`, keep worktree + artifacts, report, and (for
    auto-pick) move to the next candidate; (for user-specified) stop and report.

### Stage E — package-patch (subagent)
Set `package = "in-progress"`. Spawn `package-patch` with the three JSONs and the
`worktree`. It writes `patch.diff` + `PR.md`. Copy the regression test from the
worktree into `<work-dir>/<problem-id>/regression/`.

### Stage F — finalize the problem
Confirm `patch.diff`, `PR.md`, `regression/`, and all stage JSONs are present in
`<work-dir>/<problem-id>/`. Set `package = "done"`, `outcome = "packaged"`. **Only
now** remove the worktree (`git worktree remove`) and record the problem in
`run.json.completed_problems`. If any artifact is missing, do **not** remove the
worktree — report and stop so the problem can be resumed.

## Interruption safety

Never delete a worktree unless the problem's `outcome` is set and its artifacts are
persisted. A process/network interruption therefore leaves a resumable
worktree + `state.json` behind; the next run picks it up in *Setup / resume*.

## Report

When the loop ends, summarize: problems packaged (with `patch.diff` / `PR.md` paths),
any abandoned or gave-up problems with reasons, and — if a user-specified test stopped
the run — the verdict and `suggested_direction` so the user can decide next steps. Do
not commit, push, or open PRs.
