---
name: fix-conformance
description: Fix a not-yet-passing Asterinas conformance test (LTP / gvisor / kselftest / xfstests) and enable it. Use when the user names a blocklisted or failing conformance test to fix, asks to fix conformance tests without naming one, asks to enable/unblock a conformance test, or asks to resume an interrupted conformance fix.
---

# fix-conformance

Fix a Linux compatibility defect and enable its conformance test, one commit per test.
A test blocked because the corresponding Linux implementation also rejects the operation stays blocked.
Selection and diagnosis follow [linux-baseline.md](linux-baseline.md): source first, comparable host
validation only when practical. A pass on an arbitrary Linux environment is not the baseline.
Establish and fix the mechanism behind the violated contract, including relevant boundary and
lifecycle behavior. Match Linux's observable semantics using Asterinas's own design.

You are the **orchestrator**. Each phase runs in its own subagent and you see only its JSON
result — do not read test sources, kernel code, or QEMU logs yourself.

## Invocation

```
/fix-conformance [round=N] [<suite>] [<test>] [<gvisor-filter-term>...]
/fix-conformance resume [<run_id>]
```

Arguments are positional after `round=N`, all optional. Whatever you are not given, choose:

| Argument | When omitted |
|---|---|
| `round=N` | `1` |
| `<suite>` | phase 1 tries `ltp`, `gvisor`, `kselftest`, `xfstests`; first with a candidate wins |
| `<test>` | phase 1 picks one |
| `<gvisor-filter-term>...` | gvisor only: phase 1 picks the scope inside the chosen binary |

A `<test>` with no `<suite>` is ambiguous — ask which suite rather than guessing.

Filter terms are gvisor-only and **positive** — a case name or a wildcard like `EpollTest.*`, per the
`CONFORMANCE_TEST_GVISOR_FILTER` contract in the root `Makefile`. Join them with `:` into one
`gvisor_filter` string and pass it through untranslated; never expand or reorder them.

`round=N` counts runs that **finish**, not fixes that land: `committed` counts, and so does
`abandoned` with a verdict. See [Rounds](#rounds).

`resume` takes over an interrupted run instead of starting one. Without it you always start fresh,
even when interrupted runs exist.

## Layout

`repo_root` is the absolute path of the primary Asterinas checkout, identified during setup.
Run records and temporary worktrees live under that checkout, independent of its location on disk.

```
<repo_root>/.fix-conformance/
  select.cache diagnose.cache   # shared across runs, append-only; see cache.md
  <run-id>/                     # run state, one directory per run
    state.json                  # which phases are done, and which test this run is on
    select.json diagnose.json fix.json verify.json commit.json
    patch.diff                  # checkpoint during fix; complete verified patch after commit
<repo_root>/.worktrees/<run-id>/         # temporary; removed when the run finishes
  .osdk/bin/cargo-osdk         # this run's own OSDK
```

The two `.cache` files sit **beside** the run directories rather than inside one: they outlive the run
that wrote them. Keep run directories as evidence after reclaiming worktrees and branches.
Only `select` and `diagnose` use the caches, and only as advisory input — [cache.md](cache.md) has the rules. You never read or write them yourself.

`state.json`:

```json
{ "run_id": "run-a3f9c1", "suite": null, "test": null, "gvisor_filter": null,
  "repo_root": "<absolute primary checkout path>",
  "worktree": "<repo_root>/.worktrees/run-a3f9c1",
  "osdk_bin": "<repo_root>/.worktrees/run-a3f9c1/.osdk/bin",
  "branch": null, "base_commit": "<full main commit ID>", "vnc_port": 3417,
  "round": 1, "rounds_total": 1,
  "phases": { "select": "pending", "diagnose": "pending", "fix": "pending",
              "verify": "pending", "commit": "pending" },
  "outcome": "in-progress", "cleanup": "pending", "cleanup_head": null }
```

`outcome` is `in-progress`, `committed`, or `abandoned`. Phase status is `pending`, `running`, or
`done` — a phase left `running` was interrupted and must re-run. `suite`, `test`, `gvisor_filter`,
and `branch` stay null until phase 1 fills them in; never rename anything on disk to match.
`cleanup` is `pending`, `running`, or `done`, independent of the outcome, and may run only after a
terminal outcome. `cleanup_head` records the checked full HEAD before resource removal.
Write state updates to a temporary file in the run directory and atomically rename it to `state.json`.
Store resolved absolute paths, not literal placeholders or shell variables.
Each phase loads `REPO_ROOT` from this state; never rediscover it from that phase's working directory.

`test` is the subject of the suite's selector token — a testcase id for LTP, a *binary* for gvisor,
`<collection>:<case>` for kselftest, a test id for xfstests. `gvisor_filter` narrows within it, and is
null for every suite but gvisor, which is the only one that scopes below its selector token.

Store `gvisor_filter` as the **string** you pass to `CONFORMANCE_TEST_GVISOR_FILTER`, not a list: the
user may give a wildcard like `EpollTest.*`, and a list would silently drop the pattern. `select.json`'s
`pool_entries` holds the concrete blocklist lines it resolved to.

`branch` is the branch phase 1 created. Keep it here rather than rebuilding it from `suite`/`test`,
because retiring a run removes the worktree *before* deleting the branch — after that there is nothing
left to ask. Preserve these fields after cleanup; they then describe removed resources.
`base_commit` is the full object ID of the `main` commit this run branched from: `main`
moves while long runs are in flight, and base plus final patch must suffice to reproduce the change.
`osdk_bin` is this run's OSDK, which every `make` needs on `PATH`.

To list runs — also how you find one to resume — use `REPO_ROOT` from setup:

```sh
for d in "$REPO_ROOT"/.fix-conformance/*/; do
  [ -d "$d" ] || continue
  jq -r '"\(.run_id)  \(.suite // "-")/\(.test // "-")\(if .gvisor_filter then "  filter=\(.gvisor_filter[0:40])" else "" end)  \(.outcome)"' "$d/state.json" 2>/dev/null \
    || echo "$(basename "$d")  <unreadable state.json>"
done
```

```
run-a3f9c1  gvisor/epoll_test  filter=EpollTest.CycleOfOneDisallowed:Epol  in-progress
run-g6qN80  ltp/truncate02  committed
```

Keep the filter and the `|| echo` fallback: two gvisor runs on the same binary are otherwise
indistinguishable, and a run interrupted mid-write has truncated JSON that must read as one broken run
rather than aborting the listing.

## Phases

| Phase | File | Runs in |
|---|---|---|
| 0 setup | inline, below | you |
| 1 select | [phases/select.md](phases/select.md) | subagent |
| 2 diagnose | [phases/diagnose.md](phases/diagnose.md) | subagent |
| 3 fix | [phases/fix.md](phases/fix.md) | subagent |
| 4 verify | [phases/verify.md](phases/verify.md) | subagent |
| 5 commit | [phases/commit.md](phases/commit.md) | subagent |

Hand a subagent exactly three **absolute** paths — its phase file, this skill's
[suites.md](suites.md), and its run directory — and tell it to read all three first. It reads its
inputs from disk and writes its output JSON there. Never paste a phase's prose into the prompt, and
never summarize one phase's findings for the next: the JSON on disk is the interface.

Three paths, not four: **select** and **diagnose** also read and append to a shared cache, but their
phase files name it and link [cache.md](cache.md) themselves. Adding it to the prompt would duplicate an
instruction that already lives where it belongs.

<a id="make-prefix"></a>**Make prefix.** Every `make` any phase runs — `run_kernel`, `initramfs`,
`check`, `format` — takes all three of these, read from `state.json`:

```sh
PATH="<osdk_bin>:$PATH" VNC_PORT=<vnc_port> make <target> CARGO_OSDK="<osdk_bin>/cargo-osdk" ...
```

Dropping `PATH` or `CARGO_OSDK=` fails the build on a cargo lockfile collision that has nothing to do
with the test; dropping `VNC_PORT` kills the boot when another run is up.

## 0. Setup

### Resolve repository paths

Before starting or resuming a run, locate the primary checkout from Git's worktree list:

```sh
REPO_ROOT=$(git worktree list --porcelain -z | python3 -c '
import os, sys
print(os.fsdecode(sys.stdin.buffer.read().split(b"\0", 1)[0]).removeprefix("worktree "))
')
if ! { test -f "$REPO_ROOT/Cargo.toml" && test -d "$REPO_ROOT/kernel" && test -d "$REPO_ROOT/osdk"; }; then
    echo "Git did not identify the primary Asterinas checkout" >&2
    exit 1
fi
cd "$REPO_ROOT"
```

Git lists the primary checkout first in the standard repository layout.
Validate that path before creating or resuming a run.
This also works when invoked from a subdirectory or linked worktree;
`git rev-parse --show-toplevel` inside a run's worktree would instead select that temporary tree.
Keep `REPO_ROOT` for all rounds and persist it as `repo_root` in each run's state.
Set `RUN_DIR` to the absolute `<repo_root>/.fix-conformance/<run-id>` when the run is chosen or minted.
Resolve phase-file and `suites.md` paths from the skill being used, not from the temporary worktree.

**Without `resume`, go straight to [minting a new run](#minting-a-new-run)** — a bare
`/fix-conformance` starts fresh even when interrupted runs exist.

### `resume`

Locate the run under `REPO_ROOT/.fix-conformance` and check its saved `repo_root` against setup's root.
For legacy state without that field, infer it from the run directory's location and persist it only
after the liveness/ownership checks. If saved paths refer to a different checkout or a moved repository,
report the mismatch instead of silently rewriting them or acting on resources at the old location.

**With a `run_id`**, that run is the answer; never substitute another:

- `committed` or `abandoned` → phases already finished. Re-run no phase. If cleanup is unfinished
  (or missing in legacy state), finish [cleanup](cleanup.md), validating the archive first.
  Report its saved result and cleanup status in the [final report](#final-report) shape, then stop.
- `in-progress` → resume it, subject to the liveness check below.
- no such directory → say the id does not exist, list what does, stop.

**Without one**, take the most recently touched `in-progress` stalled run, and **name the other
stalled ids** in your first report so the user can pick. If none is resumable, say so and stop — do
not start a new run, since `resume` asked for existing work.

An `in-progress` run may be **live**: another agent is running it, and adopting it would corrupt
both. Its newest file is the heartbeat:

```sh
find "$RUN_DIR" -type f -mmin -30 | head -1   # non-empty ⇒ live
```

Use `-mmin`, not `-newermt` — `find` here is `bfs`, which rejects a relative `-newermt` and returns
empty, reading every live run as stalled.

Live belongs to another agent: leave it alone, say so, and stop if the user named that id. Stalled is
yours — report which phases are `done` and re-enter at the first that is not. Never redo a `done`
phase; the worktree still holds its build tree and `patch.diff`.

Read everything else off disk rather than asking: `state.json` has `repo_root`, worktree, `osdk_bin`, branch,
`vnc_port`, and round position; `select.json` has the test, scope, and `run_vars`. Two things need
re-checking first, since neither survives an interruption reliably:

- The in-progress worktree must still exist; normal cleanup never removes it. If it is missing,
  report the missing resume environment instead of silently replacing it and losing phase state.
  If only `osdk_bin` lacks `cargo-osdk`, reinstall as in [minting](#minting-a-new-run).
- `vnc_port` may have been taken since. Re-test it, pick a new one if busy, write it back.

If `round < rounds_total`, finish this run and then carry on with the remaining rounds.

### Minting a new run

```sh
cd "$REPO_ROOT"
EXCLUDE_FILE=$(git rev-parse --path-format=absolute --git-path info/exclude)
rg -qxF '.fix-conformance/' "$EXCLUDE_FILE" || echo '.fix-conformance/' >> "$EXCLUDE_FILE"
mkdir -p .fix-conformance .worktrees
RUN_DIR=$(mktemp -d "$REPO_ROOT/.fix-conformance/run-XXXXXX")
RUN_ID=$(basename "$RUN_DIR")
WT="$REPO_ROOT/.worktrees/$RUN_ID"
git worktree add -q --detach "$WT" main
```

- Mint the id with `mktemp -d`, not a timestamp — two runs starting in the same second would share
  an id, and every path here is keyed by it.
- Keep `WT` **absolute**: phases receive it via `state.json` and run from their own directories.
- Branch from the local `main` and do **not** fetch. The user's `main` is the intended base.
- Leave the worktree on **detached HEAD**. Phase 1 names the branch once, correctly, after it knows
  the test. Record the full `base_commit` from `git -C "$WT" rev-parse HEAD`.

Then give the run **its own OSDK** — required, not an optimization:

```sh
cd "$WT" && CARGO_INSTALL_ROOT="$WT/.osdk" OSDK_LOCAL_DEV=1 cargo install cargo-osdk --path osdk
```

Never install without `CARGO_INSTALL_ROOT`: that overwrites the shared `~/.cargo/bin/cargo-osdk` and
breaks every other tree, including the user's. Budget a few minutes — it builds OSDK from source.

Each run also builds **its own** `target/` from scratch. Do not copy, hardlink, or
`CARGO_TARGET_DIR`-share the main tree's build directory into a worktree, however many minutes it
looks like it would save: a verdict on a kernel change is only trustworthy if the build tree behind it
belongs to that run.

Then pick this run's **VNC port**, since QEMU's default is fixed and a second QEMU on it exits with
`Address already in use`:

```sh
VNC_PORT=$(shuf -i 100-9999 -n 1); until ! ss -ltn | grep -q ":$((5900+VNC_PORT))\b"; do VNC_PORT=$(shuf -i 100-9999 -n 1); done
```

Write `state.json` in `RUN_DIR` in the shape above: run id, `repo_root`, worktree, `osdk_bin`, `vnc_port`, `base_commit`, round
position, `branch: null`, all five phases `pending`, `cleanup: "pending"`, and `cleanup_head: null`.

Done when: `state.json` exists (or the resume point is identified), the worktree is on a detached HEAD
with a working `.osdk/bin/cargo-osdk`, and `git status` in the main tree is clean.

If a build fails on a cargo lockfile collision, check that `workspace.exclude` in the **main tree's**
`Cargo.toml` still lists `.worktrees` — that entry is what lets an in-repo worktree build, and cargo
reads the main tree's copy, not the worktree's.

## Driving the phases

Run phases 1 → 5 in order. Set each to `running` before, `done` after.

**A phase's result is the JSON file it wrote, not what its subagent returned.** A subagent can stop
mid-work and still return prose that reads like a result, so read the file and require it to parse
before you advance or branch on anything:

```sh
jq -e . "$RUN_DIR/<phase>.json" > /dev/null 2>&1 \
  && echo ok || echo "no usable output"
```

Keep the `2>&1`: without it a missing file prints a `jq` error that reads like a tool failure rather
than the answer to this check. Missing, empty, or truncated all mean the phase did not finish, whatever
its message said. Leave the phase `running`, and prefer **resuming that same subagent** — tell it to
complete its phase file's remaining steps and write its output — over launching a new one: its context
still holds the host baseline, boot results, and reasoning a fresh subagent would have to pay for again.
Only start a new subagent for that phase if resuming is not possible, and never mark a phase `done` on
the strength of its returned text.

With the file in hand, read only that JSON — never the subagent's prose — and branch:

- **select** returns `selected` → name the branch, suffixed with the run id:

  ```sh
  git -C "<worktree>" switch -c fix-<suite>-<test>-<run-id suffix>   # e.g. fix-ltp-truncate02-a3f9c1
  ```

  The suffix is required: without it a second run on the same test collides on the branch name. Keep
  the name at binary granularity even for a filtered gvisor run — `gvisor_filter` records the scope.
  Record `suite`, `test`, `gvisor_filter`, `branch` in `state.json`.
- **select** returns `pool_exhausted` → set `outcome` to `abandoned`, clean up even though no branch
  was created, report the exhausted pools, and stop.
- **diagnose** returns `already-green` → the pool entry is stale. Skip **fix** (mark it `done` with
  `"skipped": "already-green"`) and go to **verify**.
- **diagnose** returns `missing-feature` or `no-bug` → if the user already gave an explicit direction
  to continue despite that verdict, run **fix** with it. Otherwise retire the run. If the user named
  this test, report the verdict with its evidence and stop; if auto-picked, follow [Rounds](#rounds).
- **fix** or **verify** returns `needs-diagnosis` → reset **diagnose** and all downstream phases to
  `pending` and re-derive the baseline before any further fix or pool edit. Preserve the reason and
  checkpoint for diagnose; invalidate old downstream results so they cannot satisfy a later phase.
- **fix** returns `budget-exhausted` → retire, report what the rounds ruled out, and for an
  auto-picked test proceed according to [Rounds](#rounds).
- **verify** returns `regressed` → back to **fix** (reset that phase to `pending`) with the
  regression as the new failure to explain.
- **verify** returns `bad-pool-edit` → re-run **verify**, not **fix**: its own blocklist edit removed
  more than `pool_entries`, and the kernel change is not implicated. If a second **verify** returns it
  again, stop and report rather than looping — the pool entry itself is likely mis-resolved.
- **commit** returns with a verified final patch → set `outcome` to `committed` and
  [clean up](cleanup.md) the worktree and branch before reporting or starting another round.

Retiring a run means setting `outcome` to `abandoned` and following [cleanup.md](cleanup.md).
Both terminal outcomes reclaim the worktree and branch while preserving the run directory.
Interrupted or unresolved phases stay `in-progress` and retain their resume environment.

## Rounds

`round=N` (default 1) counts runs that **finish**. A run finishes by producing a report: `committed`,
or `abandoned` with a verdict. Only an interruption does not count.

After each run reaches an outcome, increment and — if below N — mint a fresh run and start again at
phase 0. Later rounds always auto-pick: a named test or filter applies to round 1 only. Pass nothing to
**select** but the round position — it reads the earlier rounds' outcomes off disk itself and prefers
ground they did not cover. Stop early if the pools run dry, and say how many rounds completed.

## Final report

Per committed run, report the **full `base_commit` and final `patch.diff` path** as the durable
reproduction inputs, plus the archived commit ID/subject, root cause, and **verify**'s evidence.
State whether cleanup removed the worktree and branch, or which resource remains and why.
The recorded worktree and branch are historical metadata after cleanup, not the delivered fix;
the unreferenced commit may eventually be garbage-collected, so do not rely on cherry-picking it.

Include reproduction commands using the actual base and absolute patch path:

```sh
git -C "<repo_root>" worktree add --detach "<new-review-worktree>" <full-base-commit>
git -C "<new-review-worktree>" apply --check "<absolute-run-directory>/patch.diff"
git -C "<new-review-worktree>" apply "<absolute-run-directory>/patch.diff"
```

Include `select.json`'s configuration/selector and the verification command, with the per-worktree
OSDK setup and an available VNC port needed to rerun it. Do not automatically create this review tree.
If `main` has moved, say the patch was verified against the recorded base; applicability to a newer
base requires checking. For filtered gvisor runs, name the cases enabled and those still blocked.

Per abandoned run, report the verdict, its source evidence and host-validation limitations,
what the attempts ruled out, and cleanup status. Label any retained checkpoint as unverified.
For `round=N`, report the mix plainly; one fix and two verdicts complete `round=3`.

Do not push. Do not open a PR.
