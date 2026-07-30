# fix-conformance

Fixes a conformance test that Asterinas does not yet pass, and enables it — one commit per test.

This file is for **people**. The agent reads [SKILL.md](SKILL.md); you do not need to.

## Invoking it

```
/fix-conformance [round=N] [<suite>] [<test>] [<gvisor-filter-term>...]
/fix-conformance resume [<run_id>]
```

Everything in the first form is optional, and each argument narrows the next:

```
/fix-conformance                        # pick a test and fix it
/fix-conformance ltp                    # ...from LTP
/fix-conformance ltp truncate02         # ...this test
/fix-conformance round=3                # three runs, all auto-picked
```

Omit `<suite>` and it tries `ltp`, then `gvisor`, then `kselftest`, then `xfstests`, taking the first
with a candidate. Omit `<test>` and it picks one that looks like a single localized defect. Plain
English works too: "fix a blocked LTP test", "unblock `truncate02`".

**`round=N` counts finished runs, not fixes.** A run that ends in a report counts, including one
that concludes the test needs a feature Asterinas lacks. So `round=3` means three verdicts, of which
some may be fixes — which is what makes N reachable against a pool of hard tests instead of looping
forever. Default is 1.

**gvisor takes cases, not just binaries.** A binary like `epoll_test` has several blocked gtest
cases, and one run can target any subset:

```
/fix-conformance gvisor epoll_test EpollTest.CycleOfOneDisallowed EpollTest.CycleOfThreeDisallowed
```

Terms are joined with `:` and passed to `CONFORMANCE_TEST_GVISOR_FILTER` untouched, so a wildcard like
`EpollTest.*` works exactly as it does there — matched against that binary's *blocked* cases, since
those are the lines a run can enable. Those two get fixed and the binary's other blocked cases stay
blocked, with the commit message saying so, so a reviewer seeing 2 of 5 blocklist lines removed does not
have to guess. Name only the binary and the agent picks the scope: every blocked case if they look like
one shared defect, a subset if not.

Terms are positive only, matching what `CONFORMANCE_TEST_GVISOR_FILTER` accepts.

## Resuming

```
/fix-conformance resume              # the most recent interrupted run
/fix-conformance resume run-a3f9c1   # this one specifically
```

The `resume` keyword is required. A bare `/fix-conformance` always starts fresh, even when
interrupted runs exist — it will not silently adopt one.

Passing a `run_id` is exact, and if that run already finished, you get its result off disk rather
than a re-run. With several interrupted runs and no id, it takes the most recent and tells you the
other ids. A run another agent is actively working is left alone rather than adopted.

A resumed run reuses its own worktree, so its build tree and in-progress patch are still there; only
the phases that never finished re-run.

## What you get

A **git worktree** with one commit on its own branch — the kernel fix and the blocklist edit
together, since that is how this repo lands them. Nothing is pushed and no PR is opened.

The commit message states the rule that was broken and the layer that now enforces it, so you can judge
whether the fix belongs there instead of reverse-engineering that from the diff.

```sh
git worktree list                                        # find it
cd /root/asterinas/.worktrees/run-a3f9c1 && git show      # review the commit
```

Merge it, or `git cherry-pick` it onto your own branch. When you no longer want it:

```sh
git worktree remove --force /root/asterinas/.worktrees/run-a3f9c1
git branch -D fix-ltp-truncate02-a3f9c1
```

Worktrees live in `.worktrees/`, covered by `.gitignore` and by `workspace.exclude` in the root
`Cargo.toml` — see [Design notes](#design-notes) for what that entry is doing.

## Watching a run

Every run keeps its state at the repo root, one directory per run:

```sh
for d in .fix-conformance/*/; do jq -r '"\(.run_id)  \(.suite // "-")/\(.test // "-")\(if .gvisor_filter then "  filter=\(.gvisor_filter[0:40])" else "" end)  \(.outcome)"' $d/state.json 2>/dev/null \
  || echo "$(basename $d)  <unreadable state.json>"; done
```

```
run-a3f9c1  gvisor/epoll_test  filter=EpollTest.CycleOfOneDisallowed:Epol  in-progress
run-g6qN80  ltp/truncate02  committed
run-zz1122  ltp/mmap16  abandoned
```

Those run ids are what you pass back to resume one.

Two files sit beside the run directories rather than inside one:

```sh
.fix-conformance/select.cache      # what candidates were measured to do, and on which commit
.fix-conformance/diagnose.cache    # one line per verdict, pointing at the run that explains it
```

These are shared across runs, and they exist because a single `select` boot measures dozens of
candidates while the run uses one. Without them that evidence would be stranded in one run's
`select.json`. Both are append-only JSONL, safe to `cat` or `jq` at any time, and safe to delete — a
missing cache costs re-measurement, never correctness. See [Design notes](#design-notes).

Inside each directory, one JSON file per phase records what that phase concluded — including
`diagnose.json`, which holds the root cause, and `fix.json`, which lists every attempt and what
it ruled out. An **abandoned** run is worth reading: it means the agent decided the test needs a
subsystem Asterinas lacks, or that the test itself is at fault, and `diagnose.json` says why.

Run state is excluded via `.git/info/exclude`, so it never shows up in `git status` and is never
committed. That also means it is local to your machine.

## Cleaning up

Every run builds its own `target/` rather than sharing yours, so worktrees are not small.

```sh
ls .fix-conformance                                              # what exists
git worktree list                                                # which still hold a build tree
git worktree remove --force /root/asterinas/.worktrees/<run-id>  # reclaim disk, keep the record
rm -rf .fix-conformance/<run-id>                                 # drop the record too
rm .fix-conformance/*.cache                                      # drop the shared measurements
```

Removing a worktree does not delete its run directory — the diagnosis survives, which is what
keeps a later run from re-deriving a verdict this one already paid for.

The two `.cache` files are not inside any run directory, so dropping runs leaves them behind. That is
deliberate: they are the cheapest thing to keep and the most expensive to rebuild. Deleting them is
safe whenever you want a clean slate — the next run just re-measures.

## How it works

Setup plus five phases, each of the five in its own subagent with its own context. The orchestrator
reads only the JSON file each phase writes, never the test sources or QEMU logs, which is what keeps a
long run from exhausting one context window. It reads the *file* rather than what the subagent said,
because a subagent can stop mid-work and still return something that reads like a result — that
distinction cost a re-run once.

| Phase | Does |
|---|---|
| setup | Mints the run id, worktree, per-run OSDK, and VNC port |
| select | Picks the test (and for gvisor, which of its cases), resolves its pool entries and run configuration; reads and extends `select.cache` |
| diagnose | Runs the test on host Linux for a baseline, reproduces the failure in Asterinas, finds the root cause; reads and extends `diagnose.cache` |
| fix | Up to 5 attempts, each from a new hypothesis, patching the layer that owns the broken rule |
| verify | Enables the test, then runs the full suite to check for regressions |
| commit | `make format && make check`, then one commit |

Expect it to take a while: a cold kernel build and an OSDK build per run, a QEMU boot per fix attempt,
and one full-suite run at the end.

The load-bearing idea is in **diagnose**: every suite ships prebuilt binaries that run on both
host Linux and Asterinas. So "what should the kernel do here" is never guessed — it is a diff
against the same binary's real output on Linux.

**The fix is aimed at the layer that owns the rule, not at the test.** Making a test go green is easy
to do badly: branch on the exact flag or errno the test uses and it passes while the same bug stays
reachable from everywhere else. So diagnose states the broken invariant *without mentioning the test*,
names the layer that must enforce it, and reads how Linux enforces the same rule; fix then has to work
for every caller, and records in `generalizes` why it is not a special case. Smallest diff is the
tiebreaker among correct fixes, never the goal. Where Asterinas's abstractions do not line up with
Linux's, the fix is designed against Asterinas's own and says which — Linux is the reference for what
correct means, not a structure to transplant.

Two exits skip work rather than doing it: a test that already passes means the blocklist entry is
merely stale (fix is skipped, only the entry changes), and a verdict of `missing-feature` or
`no-bug` retires the run instead of attempting a fix that cannot succeed.

## Design notes

Why the pieces are the way they are — none of this is needed to use the skill.

**Runs are resumable because every phase writes to disk before the next starts.** A resumed run picks
up at the first phase that never finished — no repeated builds, no repeated diagnosis. Untouched for
30 minutes counts as stalled and resumable; actively being written to means another agent owns it.

**Parallel runs are safe, including two on the same test,** because every path and ref is keyed by an
immutable run id: separate worktrees, state, branches, VNC port, and OSDK. Nothing here is a mutex —
there is nothing left to contend over. The branch name carries the run id suffix for the same reason;
without it a second run on the same test would collide on a name the first already holds.

**An auto-pick reads the other runs; a fully specified one does not.** When something is left to
choose, the agent lists every run's suite, test, and outcome — plus, for the unfinished ones, whether
they are still being written to — and prefers untouched ground: a test that is committed, actively
running, or sitting in a stalled run someone may resume is work already paid for. An `abandoned` run is
the interesting case, since its `diagnose.json` holds a verdict, so the agent reads that before deciding
whether the verdict was wrong. Even then it weighs rather than filters — a same-test pick is legal by
design, and this check exists to keep `round=N` from spending every round on one test, not to lock
anything.

Specify the scope fully and none of that runs: the agent resolves your entry and gets to work rather
than second-guessing you, even if another run already took it or abandoned it. Fully specified means
suite plus test — plus the cases too for gvisor, where naming only the binary still leaves the agent to
choose which of its blocked cases this run takes.

**Two phases share a cache, because a `select` boot measures far more than one run uses.** Picking a
test means running candidates on the host and probing dozens in one boot; that is most of the time a
round spends before any kernel code is read. `select.cache` keeps those measurements — how each candidate
behaved on the host, how it behaved in Asterinas, and on which commit — and `diagnose.cache` keeps one
line per verdict. A `missing-feature` verdict cached from an earlier run can retire a later one before it
builds anything.

Both are append-only JSONL, one record per line, and that format *is* the concurrency design. Concurrent
one-line appends do not interleave, so there is no lock and nothing to contend over — the same reason
nothing else in this skill needs one. Readers copy the file before parsing, since parsing it live
occasionally catches a half-written line. Every record carries the commit it was observed on, so a stale
`fail` gets re-probed rather than believed: fixing `dup03` turned `dup06` and `pipe06` green as a side
effect, which is exactly how a cached failure goes wrong.

The caches hold **measurements, not conclusions**. A candidate's probe result is something any run would
reproduce; which candidate is the best pick is one run's reasoning, and caching that would have the next
run inherit it. That distinction is deliberate — two runs on the same test exist to compare how they
reason, which only works if each does its own reasoning. And nothing depends on a cache existing: delete
either file and the next run re-measures.

**Each run installs its own OSDK,** which is most of why a run takes a few minutes to start.
`cargo-osdk` bakes its dependency paths in at compile time, so the globally installed one — built from
the main tree — hands a worktree a build mixing that worktree's `kernel` with the main tree's `ostd`.
Cargo sees one crate twice and refuses. The per-run install goes to `<worktree>/.osdk` and never
touches `~/.cargo/bin`.

**Each run builds its own `target/` too,** rather than hardlinking or sharing yours. The saved minutes
are real and the tradeoff is still not worth it: a verdict on a kernel change means something only if
the build tree behind it belongs to that run.

**Worktrees live in the repo, and that works because of one committed line.** `workspace.exclude` in
the root `Cargo.toml` lists `.worktrees`; without it cargo walks up from a worktree's crate, finds the
repo's own manifest, and lets it claim that crate as a member — the same duplicate-crate error. Cargo
reads the *main tree's* copy of that manifest, not the worktree's, so a worktree branched from an older
`main` still builds fine, while switching the main tree to a branch without the exclude breaks all of
them at once.

**Each run branches from your local `main` and never fetches.** Your `main` is the intended base;
fetching would either fail without credentials or move the base out from under the review you expected.
The run records the exact commit it branched from and reports it, so if `main` moved during a long run
you can see what the commit actually sits on and decide whether to rebase.

**`round=N` counts finished runs rather than landed fixes** because a pool can hold many tests that
turn out to need a subsystem Asterinas lacks. If only fixes counted, `round=3` against such a pool
would burn through the candidate list without ever terminating.
