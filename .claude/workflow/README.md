<!-- SPDX-License-Identifier: MPL-2.0 -->

# Conformance Fix Workflow (prototype)

A Dynamic Workflow that takes conformance test cases Asterinas currently **does
not pass** (the ones disabled in the suite *blocklists*) and, for each one,
automatically: extracts the Linux spec → diffs it against the Asterinas
implementation → locates the defect → proposes a patch → defines a regression
guard → **builds and runs the test to verify it actually passes** → drafts a PR.

## Architecture mapping

| SVG box                 | Workflow phase | What runs                                                            |
|-------------------------|----------------|---------------------------------------------------------------------|
| Task Orchestrator       | `Select`       | Pick blocklisted cases (from `args.cases` or by scanning blocklists) |
| SyscallSpecAgent        | `Spec`         | Linux semantics: params, errno, boundaries, permission model        |
| BugMiningAgent          | `Analyze`      | Diff Linux vs Asterinas, locate defect, cite `file:line` evidence    |
| PatchGenerationAgent    | `Patch`        | Concrete unified diff + blocklist lines to remove                    |
| RegressionTestAgent     | `Regression`   | Minimal repro + regression guard (un-blocklist the case)             |
| Execution Sandbox       | `Verify`       | Apply + build + run in an isolated git worktree; only PASS survives  |
| Review Gate / PRDraft   | `Synthesize`   | PR draft for verified fixes + consolidated Markdown report           |

Cases flow through `Spec → Analyze → Patch → Regression → Verify` as an
independent **pipeline**: a slow build for one case never blocks another. Stages
short-circuit gracefully — an unimplemented/too-large case still produces a
report entry, it just won't get a verified patch.

## Running it

This is a prototype script (not a registered named workflow), so invoke it by
path with the `Workflow` tool:

```jsonc
// analyze 2 auto-picked gVisor cases, build+verify each
{ "scriptPath": ".claude/workflow/conformance-fix.workflow.js",
  "args": { "suite": "gvisor", "limit": 2 } }

// target a specific case
{ "scriptPath": ".claude/workflow/conformance-fix.workflow.js",
  "args": { "suite": "gvisor", "cases": ["open_test/OpenTest.OTrunc"] } }

// dry analysis only (skip the expensive build/run)
{ "scriptPath": ".claude/workflow/conformance-fix.workflow.js",
  "args": { "suite": "gvisor", "limit": 3, "verify": false } }
```

To register it as a named workflow (invocable via `{ "name": "conformance-fix" }`),
copy or symlink it into `.claude/workflows/`.

### `args`

| key      | type       | default    | meaning                                                  |
|----------|------------|------------|----------------------------------------------------------|
| `suite`  | string     | `"gvisor"` | `gvisor` \| `ltp` \| `kselftest` \| `xfstests`           |
| `cases`  | string[]   | —          | Explicit targets like `binary/Suite.Case`; else auto-pick |
| `limit`  | number     | `2`        | How many cases to auto-pick when `cases` is omitted      |
| `verify` | boolean    | `true`     | Build + run each fix; set `false` for analysis-only       |
| `stamp`  | string     | `"run"`    | Run id used in the artifact directory name               |

## Output

Artifacts are written under `.claude/workflow/artifacts/<suite>-<stamp>/`:

- `REPORT.md` — summary table, verified fixes (Review Gate input), needs-attention list.
- `<case-id>.md` — per-case Spec, Finding+evidence, Patch diff, Regression plan,
  Verification result, and PR draft.

The workflow's return value is a compact JSON summary (counts + per-case verdicts).

## Requirements & caveats

- **Run inside the project Docker dev container** (see [AGENTS.md](../../AGENTS.md)).
  The `Verify` phase runs `make run_kernel AUTO_TEST=conformance
  CONFORMANCE_TEST_SUITE=<suite> TESTS=<binary>` in an isolated worktree, so it
  needs the full toolchain + KVM/QEMU.
- **`Verify` is the expensive phase** — a kernel build + QEMU boot per case. Keep
  `limit` small; use `verify: false` to iterate on analysis quickly.
- Only fixes whose target case **passes with no new regressions** are promoted to
  the "Verified fixes" section and get a PR draft. Everything else lands in
  "Needs attention" for a human.
- gVisor test C++ sources are prebuilt binaries (not in-tree); the `Spec` phase
  reasons from man pages + case names + gVisor upstream semantics.
- **Analysis phases (`Select`..`Regression`) are read-only** — they inspect the
  repo but never edit it; the proposed fix is carried as diff *text*. Only
  `Verify` mutates code, and only inside its isolated worktree. The `report`
  agent resolves the repo root (`git rev-parse --show-toplevel`) and writes
  artifacts under it, so artifacts land in-repo regardless of the agent's CWD.

## Validation

A `verify: false` dry run on LTP `unlink08` exercised the full pipeline and
produced [artifacts/ltp-dryrun/](artifacts/ltp-dryrun/): the analysis correctly
root-caused a missing parent-directory permission check in `Path::unlink`
(asymmetric with `new_fs_child`), with `file:line` evidence, a surgical diff, and
an un-blocklist regression plan. That run also caught two prototype bugs since
fixed: artifacts written outside the repo (now anchored to the git root) and an
analysis agent editing the working tree (now forbidden via the read-only guard).
