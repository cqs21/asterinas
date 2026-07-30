# Phase 1: select

Pick the one test this problem will fix, and resolve everything needed to run it.

**In:** the suite, test, and (for gvisor) filter terms the user named, if any; `suites.md`. Only when
some part of that scope was left to you, also the other runs under `/root/asterinas/.fix-conformance/` —
every `state.json`, plus `select.json` and `diagnose.json` for the ones worth a closer look — and
`select.cache`, per [cache.md](../cache.md). Run every command from the worktree in `state.json`.
**Out:** `select.json` in the run directory.

## Do

**First decide whether anything is actually left to choose.** The scope is fully specified when the
user named the suite and the test — and, for gvisor only, the filter terms as well, since gvisor scopes
below its selector token and a binary without them still leaves the cases open.

**Fully specified — resolve, do not choose.** Find which pool holds the entry, confirm it exists, and
fill in `selector_token`, `run_vars`, `pool_file`, `pool_entries`, and `enable_action` for it. For
gvisor keep the filter string verbatim, resolve it to concrete blocklist lines, and leave the binary's
other blocked cases alone. If the entry does not exist, say so in `notes` — the test may already be
enabled. Do **not** consult the other runs in `.fix-conformance/`: a named scope is the answer even if
another run already took it, or abandoned it, and re-deciding that is not this phase's call. Then stop;
everything below is for the case where something was left to you.

**Anything unspecified — pick it, and pick only what you were not given.** The orchestrator passes
whichever of suite / test / filter terms the user named, and each one narrows what is left. A gvisor
binary named without filter terms is this case, not the one above: the binary is settled, the cases are
yours to choose, and the other runs are worth reading for that choice. Whatever the source, record the
scope as a `gvisor_filter` string plus resolved `pool_entries`.

**No suite named:** try them in this order and take the first with a usable candidate — `ltp`,
`gvisor`, `kselftest`, `xfstests`. Only move on when a suite genuinely offers nothing; a suite whose
pool is non-empty is the answer even if its candidates look hard.

**Suite named, no test:** search that suite's pools in `suites.md` priority order. For `ltp` that is
`all.txt` commented-out entries, then `blocked/ext2.txt`, then `blocked/exfat.txt`.

**gvisor binary named, no filter terms:** choose the scope yourself from that binary's blocklist — all
of its blocked cases when they look like one shared defect, a subset when they do not — and say which
in `rationale`.

### Read `select.cache` first

Whenever some part of the scope is yours to choose, read the cache before spending a single host run or
boot on candidates another run has already probed. Follow [cache.md](../cache.md) for how — snapshot,
then reduce to the newest record per key:

```sh
cp /root/asterinas/.fix-conformance/select.cache /tmp/sel.snap 2>/dev/null || : > /tmp/sel.snap
jq -s 'group_by(.kind + " " + .suite + " " + .test) | map(last)' /tmp/sel.snap
```

Three record kinds, and each buys you something different:

- `host` — how the prebuilt binary behaves on real Linux. A `tconf` or `tbroke` here removes a candidate
  from consideration without running anything, and does not go stale when Asterinas changes.
- `probe` — how it behaved in Asterinas, on the `commit` in the record. A `pass` means the pool entry is
  stale, which is the cheapest possible round. A **`fail` from an older commit must be re-probed** rather
  than trusted: an intervening fix may have turned it green as a side effect.
- `stale-entry` — a candidate a previous run observed passing but did not enable. These are the readiest
  work in the cache.

What the cache does **not** decide is which candidate you pick. It tells you what was measured; the
choice among what is left is still yours to make and to justify in `rationale`. A cache miss is normal —
probe as your phase file describes.

Then, as before, prefer ground no other run has covered. What every run is on, and whether the
unfinished ones are still moving:

```sh
cd /root/asterinas
for d in .fix-conformance/*/; do
  live=$([ -n "$(find $d -type f -mmin -30 | head -1)" ] && echo live || echo stalled)
  jq -r --arg live "$live" '"\(.suite // "-")/\(.test // "-")  \(.outcome)\(if .outcome == "in-progress" then "/\($live)" else "" end)  \(.run_id)"' $d/state.json 2>/dev/null \
    || echo "$(basename $d)  <unreadable state.json>"
done
git branch -a --list 'fix-*'   # in flight or already fixed
```

```
ltp/truncate02  committed  run-g6qN80
ltp/rename01  in-progress/live  run-a3f9c1
-/-  in-progress/stalled  run-zz1122
```

Read `outcome`, not just the test name: each state below is a different signal, and the name alone
tells you nothing. Use `-mmin`, not `-newermt` — `find` here is `bfs`, which rejects a relative
`-newermt` and returns empty, reading every live run as stalled. Keep the `|| echo` fallback so a run
interrupted mid-write reads as one broken run instead of aborting the listing.

Weigh what you find rather than filtering on it; neither list is a lock, and two runs landing on the
same test breaks nothing.

- `committed`, or `in-progress/live` — covered. Pick something else.
- `in-progress/stalled` — an interrupted run someone may `resume`. Treat as covered too: re-picking it
  duplicates work that still has a worktree and a diagnosis on disk.
- `abandoned` — the strongest signal. Read that run's `diagnose.json` first, and only re-pick if you
  have a reason its verdict was wrong; say that reason in `rationale`.
- A run whose `suite`/`test` is still `-` has not chosen yet and rules nothing out.

For gvisor, a match at binary granularity is not necessarily a collision: compare `gvisor_filter` and
`pool_entries` from that run's `select.json`, since another run on the same binary may have taken
different cases and left yours blocked.

Among what is left, favor a failure that is likely one localized defect: read candidate test sources
and prefer narrow syscall semantics over a subsystem Asterinas lacks.

Cheap way to shortlist: run a handful of candidates on the host to confirm they pass there, then run
those same tests in Asterinas in **one** boot — the selector is comma-separated — and read which fail
and how. That boot needs the full make prefix from SKILL.md (`PATH=<osdk_bin>:$PATH`, `VNC_PORT=`,
`CARGO_OSDK=`). A candidate that already passes in Asterinas is a fine pick: its pool entry is stale
and the fix is the pool edit alone.

If no candidate remains in any pool, write `{"result": "pool_exhausted"}` and stop.

### Write what you measured to `select.cache`

Append one line per candidate you actually observed, whether or not you picked it. That boot probed
dozens of candidates and the run only uses one; the rest is exactly what the next run should not have to
re-measure. Follow [cache.md](../cache.md) — one line per record, nothing over ~2KB, no pretty-printing:

```sh
printf '%s\n' "$REC" >> /root/asterinas/.fix-conformance/select.cache
```

```json
{"kind":"host","suite":"ltp","test":"dup03","result":"pass","version":"20260529","run":"run-BF0WPt"}
{"kind":"probe","suite":"ltp","test":"dup03","result":"fail","commit":"d07835db2","run":"run-BF0WPt"}
{"kind":"stale-entry","suite":"ltp","test":"dup02","result":"pass","commit":"d07835db2","run":"run-BF0WPt"}
```

- `host` — one per candidate you ran on the host. `result` is `pass`, `fail`, `tconf`, or `tbroke`, and
  `version` is the suite's pinned version rather than a commit, since this observation is about real
  Linux and does not go stale when Asterinas changes.
- `probe` — one per candidate in your Asterinas boot, `result` `pass` or `fail`, with `commit` set to
  this run's `base_commit`.
- `stale-entry` — for a still-blocked candidate you saw *pass* in Asterinas. Same information a `probe`
  carries, flagged so the next run can find the ready work without re-deriving it. Emit it for every such
  candidate, not just the one you picked.

For gvisor use the binary as `test` and add `"case"` for the gtest case, so cases stay independently
addressable. Skip candidates you never actually ran: an unmeasured test has nothing to record, and
guessing here would poison every later run.

Do **not** append your ranking or your reasoning. `rationale` belongs in `select.json`; the cache holds
measurements, so the next run makes its own choice from the same evidence rather than inheriting yours.

## Out shape

```json
{
  "result": "selected",
  "suite": "ltp",
  "test": "truncate02",
  "gvisor_filter": null,
  "selector_token": "truncate02",
  "run_vars": "CONFORMANCE_TEST_WORKDIR=/exfat",
  "pool_file": "test/initramfs/src/conformance/ltp/testcases/blocked/exfat.txt",
  "pool_entries": ["truncate02"],
  "enable_action": "delete the line",
  "source": "auto-picked",
  "rationale": "why this looks like one localized defect",
  "notes": ""
}
```

For gvisor, where the unit of work is the gtest case, `test` is the binary, `gvisor_filter` is the
filter string scoping it, and `pool_entries` holds the concrete blocklist line per case:

```json
{
  "result": "selected",
  "suite": "gvisor",
  "test": "epoll_test",
  "gvisor_filter": "EpollTest.CycleOfOneDisallowed:EpollTest.CycleOfThreeDisallowed",
  "selector_token": "epoll_test",
  "run_vars": "CONFORMANCE_TEST_GVISOR_FILTER=EpollTest.CycleOfOneDisallowed:EpollTest.CycleOfThreeDisallowed",
  "pool_file": "test/initramfs/src/conformance/gvisor/blocklists/epoll_test",
  "pool_entries": ["EpollTest.CycleOfOneDisallowed", "EpollTest.CycleOfThreeDisallowed"],
  "enable_action": "delete these lines",
  "source": "user-named",
  "rationale": "both are cycle-detection checks in epoll_ctl — likely one missing check",
  "notes": "3 other cases in this file stay blocked"
}
```

`selector_token` is the suite's token format for this test, exactly as
`CONFORMANCE_TEST_SELECTOR` wants it.

`run_vars` is every *other* variable the later phases must pass to `make run_kernel`: the
workdir and extra-blocklist pair matching the pool the entry came from, and for gvisor a
`CONFORMANCE_TEST_GVISOR_FILTER=...` when you are narrowing to specific gtest cases. Put them
here in full, so no later phase has to re-derive them from `suites.md`.

`gvisor_filter` is `null` for every suite but gvisor, and otherwise the **string** to pass to
`CONFORMANCE_TEST_GVISOR_FILTER` — colon-separated positive terms, verbatim if the user gave them. Keep
it a string, not a list: `EpollTest.*` is one term and a list would lose the pattern.

`pool_entries` is the resolved concrete lines that filter enables. Resolve patterns here — expand
`EpollTest.*` against the blocklist and list the matching lines — since later phases edit and verify
lines, not patterns. It must contain exactly the lines being enabled and no others, so the binary's
remaining blocked cases stay blocked.

Resolve against the blocklist file, not the binary's full case list: `EpollTest.*` may match 21 cases in
`epoll_test` while only 5 are blocked, and this run enables blocked lines. Verify the two agree — a
pattern matching zero blocklist lines means the scope is empty, which is a `notes` entry, not a run.

`source` is `auto-picked` or `user-named`. `enable_action` is `uncomment the line`, `delete the
line`, `delete these lines`, or for a gvisor binary absent from `TESTS`, `add to TESTS`.

Done when: `select.json` names the test (and `gvisor_filter`, for gvisor), its exact selector token,
its resolved `pool_entries`, the `run_vars` for its configuration, and how it gets enabled.
