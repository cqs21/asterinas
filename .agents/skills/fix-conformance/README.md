# fix-conformance

Fixes Linux compatibility defects exposed by conformance tests and enables the tests, one commit per test.
An entry remains blocked when the corresponding Linux implementation also rejects the tested operation.

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
than a re-run; any unfinished cleanup is completed after checking the saved patch.
With several interrupted runs and no id, it takes the most recent and tells you the other ids. A run another agent is actively working is left alone rather than adopted.

A resumed run reuses its own worktree, so its build tree and in-progress patch are still there; only
the phases that never finished re-run.

## What you get

A run directory containing the **full base commit ID and a complete `patch.diff`**, plus the diagnosis,
verification evidence, and commit metadata. The patch includes both the kernel fix and the blocklist edit,
or just the blocklist edit when the test already passes.
Nothing is pushed and no PR is opened.

The skill creates one commit in a temporary worktree, exports its full change from the recorded base,
and verifies that applying the patch reconstructs the committed tree.
It then removes the worktree and branch. Their original names stay in `state.json` as historical metadata;
you do not need them, or the final commit object, to reproduce the fix.

Use the actual base and patch path from the report to review it in a new worktree:

```sh
# From the checkout containing .fix-conformance/:
REPO_ROOT=$(git rev-parse --show-toplevel)
REVIEW_WT="$REPO_ROOT/.worktrees/review-conformance"
git -C "$REPO_ROOT" worktree add --detach "$REVIEW_WT" <full-base-commit>
git -C "$REVIEW_WT" apply --check "$REPO_ROOT/.fix-conformance/<run-id>/patch.diff"
git -C "$REVIEW_WT" apply "$REPO_ROOT/.fix-conformance/<run-id>/patch.diff"
git -C "$REVIEW_WT" diff
```

The report includes the test configuration and validation command.
Before rerunning it, install that review worktree's own OSDK and choose an available VNC port as in
[setup](SKILL.md#minting-a-new-run); then use those values in the reported command.
The saved commit message describes the root cause and owning layer; use it when committing the reviewed patch.
The original commit ID is informational because Git may garbage-collect it after its branch is deleted.

Worktrees live in `.worktrees/`, covered by `.gitignore` and by `workspace.exclude` in the root
`Cargo.toml` — see [Design notes](#design-notes) for what that entry is doing.

## Watching a run

Setup discovers the primary Asterinas checkout through Git, even when invoked from a subdirectory or
linked worktree. It stores that absolute path as `state.json.repo_root`, so phases share one location
for `.fix-conformance/` and `.worktrees/`. The checkout need not live at a fixed container path.
Absolute paths passed to phases and saved for resume are resolved from that root.

Every run keeps its state there, one directory per run. From that checkout:

```sh
for d in .fix-conformance/*/; do
  [ -d "$d" ] || continue
  jq -r '"\(.run_id)  \(.suite // "-")/\(.test // "-")\(if .gvisor_filter then "  filter=\(.gvisor_filter[0:40])" else "" end)  \(.outcome)"' "$d/state.json" 2>/dev/null \
    || echo "$(basename "$d")  <unreadable state.json>"
done
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

Run state is excluded via Git's local `info/exclude` file, whose path setup resolves through Git,
so it never shows up in `git status` and is never committed.
That also means it is local to your machine.

## Cleaning up

Finished runs automatically remove their worktrees, per-run OSDK/build output, and branches.
The run directory remains: `state.json`, phase JSON, the final patch, and saved evidence are the durable result.
An abandoned run retains its verdict and any unverified attempt checkpoint.
Interrupted runs keep their worktrees and branches so `resume` can reuse their build and phase state.

`state.json` tracks cleanup separately from the verdict: `pending`, `running`, or `done`.
If interrupted after committing or between removing the worktree and deleting the branch,
`/fix-conformance resume <run-id>` finishes cleanup without rerunning completed phases.
If archive verification or ownership checks fail, the report names the resources left in place and why.

Deleting a retained run directory is manual: it removes the patch needed to reproduce its fix and the
reasoning referenced by caches. The shared `.cache` files may be deleted independently when you want
fresh measurements; missing caches never determine correctness.

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
| diagnose | Derives the baseline from the matching Linux source, validates on a comparable host when practical, and compares Asterinas; reads and extends `diagnose.cache` |
| fix | Up to 5 attempts, each from a new hypothesis, patching the layer that owns the broken rule |
| verify | Enables the test, then runs the full suite to check for regressions |
| commit | `make format && make check`, one commit, then export and verify the complete patch |
| cleanup | Preserves the result and removes the finished run's worktree and branch |

Expect it to take a while: a cold kernel build and an OSDK build per run, a QEMU boot per fix attempt,
and one full-suite run at the end.

**Diagnosis starts from the matching Linux source.** It reads the pinned test and the Linux syscall
path through the relevant filesystem or subsystem, recording the revision, functions, expected returns,
and effects. A host run corroborates that conclusion only when its relevant configuration is comparable.
If matching the environment would require a kernel rebuild or similarly complex setup, execution is
skipped and the source evidence determines the verdict.

A filesystem's advertised mount type may differ from the driver implementing the operation.
The baseline must establish the actual implementation; see [linux-baseline.md](linux-baseline.md).

**The fix is aimed at the layer that owns the rule, not at the test.** Making a test go green is easy
to do badly: branch on the exact flag or errno the test uses and it passes while the same bug stays
reachable from everywhere else. So diagnose states the broken invariant *without mentioning the test*,
names the layer that must enforce it, and reads how Linux enforces the same rule; fix then has to work
for every caller, and records in `generalizes` why it is not a special case. Smallest diff is the
tiebreaker among correct fixes, never the goal. Where Asterinas's abstractions do not line up with
Linux's, the fix is designed against Asterinas's own and says which — Linux is the reference for what
correct means, not a structure to transplant.

For limits and resource allocation, the reported values must agree with the mechanism enforcing them.
Diagnosis and fix check the boundary and lifecycle behavior relevant to the defect, including behavior
the test may omit. Asterinas can use its own algorithms; the required match is observable semantics.
The compared paths and focused validation are recorded in the existing diagnosis and fix fields.

A test that already passes and meets the matching Linux contract has a stale blocklist entry:
fix is skipped and only the entry changes.
A verdict of `missing-feature` or `no-bug` retires the run with its evidence.

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

**Two phases share a cache, because a `select` boot measures far more than one run uses.**
After checking Linux source, selection can corroborate suitable candidates on the host and probe
candidates sharing a configuration in one Asterinas boot.
`select.cache` keeps those measurements — how each candidate behaved on the host,
how it behaved in Asterinas, and on which commit — and `diagnose.cache` keeps one
line per verdict. A `missing-feature` verdict cached from an earlier run can retire a later one before it
builds anything.

Records are scoped by configuration and gvisor case; host observations also carry the suite version
and relevant Linux environment, including driver evidence for filesystem-dependent tests.
Legacy entries without that context are leads to re-check, not proof that a blocklist is wrong.

Both are append-only JSONL, one record per line, and that format *is* the concurrency design. Concurrent
one-line appends do not interleave, so there is no lock and nothing to contend over — the same reason
nothing else in this skill needs one. Readers copy the file before parsing, since parsing it live
occasionally catches a half-written line. Every record carries the commit it was observed on, so a stale
`fail` gets re-probed rather than believed: fixing `dup03` turned `dup06` and `pipe06` green as a side
effect, which is exactly how a cached failure goes wrong.

Candidate caches hold **measurements, not rankings**; diagnosis caches hold verdicts with evidence pointers.
A candidate's probe result is something any run would reproduce; which candidate is the best pick is one run's reasoning, and caching that would have the next
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
