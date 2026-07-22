# fix-conformance

An end-to-end **conformance-fix workflow** for the Asterinas OS kernel.
It takes a conformance suite, picks a not-yet-passing (blocklisted) test, diagnoses it
against Linux semantics, fixes the kernel, writes a minimal regression test, verifies
there are no regressions, and packages a `patch.diff` + `PR.md` — one independent patch
per problem.

It is **agent-agnostic**: the same skill runs under both Claude Code and Codex, using
each agent's own subagent-spawn primitive (`Task` / `codex exec`).
It is **local-first**: no server and no PR are created; it reads the repo, works in an
isolated git worktree, and writes files.
It is **resumable**: each problem's progress is persisted to `state.json`, so an
interrupted run continues where it left off.

## What it orchestrates

Four stage skills, each runnable on its own, spawned in an **isolated subagent** with a
clean context and handed off through **JSON files** on disk:

1. [`select-conformance-test`](../select-conformance-test/) — pick a blocklisted test,
   scoring difficulty statically from the blocklist (easiest first).
2. [`diagnose-and-fix`](../diagnose-and-fix/) — compare against Linux, decide a verdict
   (`bug` / `no-bug` / `missing-feature`), and apply a minimal fix in the worktree.
3. [`regress-and-verify`](../regress-and-verify/) — write a minimal regression test,
   enable the conformance test, and run the affected suite full to prove no regressions.
4. [`package-patch`](../package-patch/) — compose the final patch (fix + blocklist
   removal only) and the PR description.

## Quick start

`fix-conformance` is a skill, not a binary — trigger it from inside an agent session,
not from a shell. It requires a **suite** and a **work directory** (persistent, holds
artifacts + resume state); `test`, `count`/`N`, and `continue_direction` are optional.

- **Claude Code:** `/fix-conformance suite=gvisor work-dir=.conformance-work`,
  or just ask: *"Fix the easiest not-yet-passing gvisor conformance test; put artifacts
  in `.conformance-work`."*
- **Codex:** *"Use the fix-conformance skill to fix LTP `rename01`, work-dir
  `.conformance-work`."*

Examples:

```
# Auto-pick the easiest gvisor failure, produce one patch.
suite=gvisor work-dir=.conformance-work

# A specific test (user-specified: stops and asks if it turns out not to be a bug).
suite=ltp test=rename01 work-dir=.conformance-work

# Fix 5 problems in one run — the whole loop runs 5×, one patch each.
suite=gvisor count=5 work-dir=.conformance-work

# Force a fix even on a no-bug/missing-feature verdict, giving a direction.
suite=gvisor test=epoll_test work-dir=.conformance-work \
    continue_direction="use a monotonic-clock deadline instead of jiffies"
```

## Prerequisites

The stages build the kernel and run conformance suites in **QEMU**, which only works
**inside the Asterinas Docker container** (see [`AGENTS.md`](../../../AGENTS.md),
"Building and Running"). Start the agent session from within that container.

The workflow relies on `CONFORMANCE_TEST_SELECTOR` / `CONFORMANCE_TEST_GVISOR_FILTER`
(from PR #3598) to run a single blocklisted test — it assumes these are available.

## What you get back

Under `<work-dir>`, one sub-directory per problem
(`<suite>__<test>`, e.g. `gvisor__epoll_test/`):

| File | What it is |
|---|---|
| `patch.diff` | The **final patch**: kernel fix + blocklist removal only. |
| `PR.md` | PR description: root-cause analysis, solution, verification evidence. |
| `regression/` | A copy of the minimal regression test — delivered **separately**, never in the patch. |
| `root-cause.md` | Human-readable root-cause analysis. |
| `select.json`, `diagnose.json`, `verify.json`, `state.json` | Structured stage hand-offs; `state.json` drives resume. |

The skill never commits, pushes, or opens a PR — it produces the files and lets a human
decide.

## Standalone stages

Every stage skill is independently useful. Most common:
[`diagnose-and-fix`](../diagnose-and-fix/) on a `file:lines` range gives you a
root-cause analysis and a fix diff without the rest of the pipeline.

## Design

The single source of truth for the directory layout, JSON schemas, subagent-spawn
model, worktree/git rules, retry/completion logic, and shared vocabulary is
[`../shared/conformance-workflow-contract.md`](../shared/conformance-workflow-contract.md).
Every stage skill reads it first; start there to understand or modify the workflow.
