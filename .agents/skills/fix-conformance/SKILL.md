---
name: fix-conformance
description: Fix a not-yet-passing Asterinas conformance test (LTP / gvisor / kselftest / xfstests) and enable it. Use when the user names a blocklisted or failing conformance test to fix, asks to fix conformance tests without naming one, asks to enable/unblock a conformance test, or asks to resume an interrupted conformance fix.
---

# fix-conformance

Turn a not-yet-passing conformance test **green** and enable it, one commit per test.

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

```
/root/asterinas/.fix-conformance/
  select.cache diagnose.cache   # shared across runs, append-only; see cache.md
  <run-id>/                     # run state, one directory per run
    state.json                  # which phases are done, and which test this run is on
    select.json diagnose.json fix.json verify.json commit.json
    patch.diff                  # the fix, rewritten after every fix round
/root/asterinas/.worktrees/<run-id>/         # this run's worktree; phases run commands here
  .osdk/bin/cargo-osdk         # this run's own OSDK
```

The two `.cache` files sit **beside** the run directories rather than inside one: they outlive the run
that wrote them, and a retired run's directory may be reclaimed. Only `select` and `diagnose` use them,
and only as advisory input — [cache.md](cache.md) has the rules. You never read or write them yourself.

`state.json`:

```json
{ "run_id": "run-a3f9c1", "suite": null, "test": null, "gvisor_filter": null,
  "worktree": "/root/asterinas/.worktrees/run-a3f9c1",
  "osdk_bin": "/root/asterinas/.worktrees/run-a3f9c1/.osdk/bin",
  "branch": null, "base_commit": "8a9543109", "vnc_port": 3417,
  "round": 1, "rounds_total": 1,
  "phases": { "select": "pending", "diagnose": "pending", "fix": "pending",
              "verify": "pending", "commit": "pending" },
  "outcome": "in-progress" }
```

`outcome` is `in-progress`, `committed`, or `abandoned`. Phase status is `pending`, `running`, or
`done` — a phase left `running` was interrupted and must re-run. `suite`, `test`, `gvisor_filter`,
and `branch` stay null until phase 1 fills them in; never rename anything on disk to match.

`test` is the subject of the suite's selector token — a testcase id for LTP, a *binary* for gvisor,
`<collection>:<case>` for kselftest, a test id for xfstests. `gvisor_filter` narrows within it, and is
null for every suite but gvisor, which is the only one that scopes below its selector token.

Store `gvisor_filter` as the **string** you pass to `CONFORMANCE_TEST_GVISOR_FILTER`, not a list: the
user may give a wildcard like `EpollTest.*`, and a list would silently drop the pattern. `select.json`'s
`pool_entries` holds the concrete blocklist lines it resolved to.

`branch` is the branch phase 1 created. Keep it here rather than rebuilding it from `suite`/`test`,
because retiring a run removes the worktree *before* deleting the branch — after that there is nothing
left to ask. `base_commit` is the `main` commit this run branched from, for the final report: `main`
moves while long runs are in flight, and a reviewer needs to know what the commit sits on.
`osdk_bin` is this run's OSDK, which every `make` needs on `PATH`.

To list runs — also how you find one to resume:

```sh
for d in .fix-conformance/*/; do jq -r '"\(.run_id)  \(.suite // "-")/\(.test // "-")\(if .gvisor_filter then "  filter=\(.gvisor_filter[0:40])" else "" end)  \(.outcome)"' $d/state.json 2>/dev/null \
  || echo "$(basename $d)  <unreadable state.json>"; done
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
PATH=<osdk_bin>:$PATH VNC_PORT=<vnc_port> make <target> CARGO_OSDK=<osdk_bin>/cargo-osdk ...
```

Dropping `PATH` or `CARGO_OSDK=` fails the build on a cargo lockfile collision that has nothing to do
with the test; dropping `VNC_PORT` kills the boot when another run is up.

## 0. Setup

**Without `resume`, go straight to [minting a new run](#minting-a-new-run)** — a bare
`/fix-conformance` starts fresh even when interrupted runs exist.

### `resume`

**With a `run_id`**, that run is the answer; never substitute another:

- `committed` or `abandoned` → already finished. Re-run nothing. Report its result from the JSON on
  disk in the [final report](#final-report) shape, say plainly that it was already complete, stop.
- `in-progress` → resume it, subject to the liveness check below.
- no such directory → say the id does not exist, list what does, stop.

**Without one**, take the most recently touched `in-progress` stalled run, and **name the other
stalled ids** in your first report so the user can pick. If none is resumable, say so and stop — do
not start a new run, since `resume` asked for existing work.

An `in-progress` run may be **live**: another agent is running it, and adopting it would corrupt
both. Its newest file is the heartbeat:

```sh
find .fix-conformance/<run-id> -type f -mmin -30 | head -1   # non-empty ⇒ live
```

Use `-mmin`, not `-newermt` — `find` here is `bfs`, which rejects a relative `-newermt` and returns
empty, reading every live run as stalled.

Live belongs to another agent: leave it alone, say so, and stop if the user named that id. Stalled is
yours — report which phases are `done` and re-enter at the first that is not. Never redo a `done`
phase; the worktree still holds its build tree and `patch.diff`.

Read everything else off disk rather than asking: `state.json` has the worktree, `osdk_bin`, branch,
`vnc_port`, and round position; `select.json` has the test, scope, and `run_vars`. Two things need
re-checking first, since neither survives an interruption reliably:

- `osdk_bin` must still hold a `cargo-osdk`. If the worktree was cleaned up under it, reinstall as in
  [minting](#minting-a-new-run) before running any phase.
- `vnc_port` may have been taken since. Re-test it, pick a new one if busy, write it back.

If `round < rounds_total`, finish this run and then carry on with the remaining rounds.

### Minting a new run

```sh
cd /root/asterinas
grep -qxF '.fix-conformance/' .git/info/exclude || echo '.fix-conformance/' >> .git/info/exclude
mkdir -p .fix-conformance .worktrees
RUN_ID=$(basename $(mktemp -d .fix-conformance/run-XXXXXX))
WT=$PWD/.worktrees/$RUN_ID
git worktree add -q --detach $WT main
```

- Mint the id with `mktemp -d`, not a timestamp — two runs starting in the same second would share
  an id, and every path here is keyed by it.
- Keep `WT` **absolute**: phases receive it via `state.json` and run from their own directories.
- Branch from the local `main` and do **not** fetch. The user's `main` is the intended base.
- Leave the worktree on **detached HEAD**. Phase 1 names the branch once, correctly, after it knows
  the test. Record `base_commit` from `git rev-parse --short main`.

Then give the run **its own OSDK** — required, not an optimization:

```sh
cd $WT && CARGO_INSTALL_ROOT=$WT/.osdk OSDK_LOCAL_DEV=1 cargo install cargo-osdk --path osdk
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

Write `state.json` in the shape above: run id, worktree, `osdk_bin`, `vnc_port`, `base_commit`, round
position, `branch: null`, all five phases `pending`.

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
jq -e . /root/asterinas/.fix-conformance/<run-id>/<phase>.json > /dev/null 2>&1 \
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
  git -C <worktree> switch -c fix-<suite>-<test>-<run-id suffix>   # e.g. fix-ltp-truncate02-a3f9c1
  ```

  The suffix is required: without it a second run on the same test collides on the branch name. Keep
  the name at binary granularity even for a filtered gvisor run — `gvisor_filter` records the scope.
  Record `suite`, `test`, `gvisor_filter`, `branch` in `state.json`.
- **select** returns `pool_exhausted` → report and stop.
- **diagnose** returns `already-green` → the pool entry is stale. Skip **fix** (mark it `done` with
  `"skipped": "already-green"`) and go to **verify**.
- **diagnose** returns `missing-feature` or `no-bug` → retire the run. If the user named this test,
  report the verdict with its evidence and stop; if it was auto-picked, start a fresh run at phase 0.
  Unless the user asked to continue anyway and gave a fix direction — then run **fix** with it.
- **fix** returns `budget-exhausted` → retire, report what the rounds ruled out, and for an
  auto-picked test restart as above.
- **verify** returns `regressed` → back to **fix** (reset that phase to `pending`) with the
  regression as the new failure to explain.
- **verify** returns `bad-pool-edit` → re-run **verify**, not **fix**: its own blocklist edit removed
  more than `pool_entries`, and the kernel change is not implicated. If a second **verify** returns it
  again, stop and report rather than looping — the pool entry itself is likely mis-resolved.
- **commit** returns → set `outcome` to `committed`.

Retiring a run means setting `outcome` to `abandoned`, then reclaiming disk while **keeping the run
directory** — `diagnose.json` holds the reasoning behind a verdict the next run should not have to pay
for again, and `diagnose.cache` only carries the one-line summary plus a pointer to it:

```sh
git -C /root/asterinas worktree remove --force /root/asterinas/.worktrees/<run-id>
git -C /root/asterinas branch -D fix-<suite>-<test>-<run-id suffix>   # if it was created
```

## Rounds

`round=N` (default 1) counts runs that **finish**. A run finishes by producing a report: `committed`,
or `abandoned` with a verdict. Only an interruption does not count.

After each run reaches an outcome, increment and — if below N — mint a fresh run and start again at
phase 0. Later rounds always auto-pick: a named test or filter applies to round 1 only. Pass nothing to
**select** but the round position — it reads the earlier rounds' outcomes off disk itself and prefers
ground they did not cover. Stop early if the pools run dry, and say how many rounds completed.

## Final report

Per committed run: worktree path, branch, the `base_commit` it sits on, root cause, and **verify**'s
evidence — plus, for a filtered gvisor run, which cases are now enabled and which of that binary's stay
blocked. If `main` has moved past `base_commit` since, say so: the commit still applies, but the user is
choosing whether to rebase. Per abandoned run: the verdict and what it ruled out. For `round=N`, report
the mix plainly; three finished runs with one fix and two verdicts is a complete answer to `round=3`,
not a partial one.

Do not push. Do not open a PR.
