# Caches

Knowledge one run paid for that the next should not pay for again.

`select` and `diagnose` each own one cache file, shared by every run:

```
/root/asterinas/.fix-conformance/select.cache      # candidate observations
/root/asterinas/.fix-conformance/diagnose.cache    # verdicts
```

They sit beside the run directories, not inside one — a cache outlives the run that wrote it, and a
retired run's directory may be reclaimed.

Read a cache at the start of your phase if it exists; append to it at the end. There is no cache to
create up front and no error if one is missing: a first run writes the file, later runs extend it.

## Advisory, never authoritative

A cache is **evidence from another run**, not a decision. Nothing in it removes a step from your phase
file, and no cache entry is a lock. Specifically:

- A cache hit that agrees with what you would have done saves the work. A cache hit you have reason to
  doubt is a hypothesis to re-check, not an answer to adopt.
- A cache miss is the normal state, not a failure. Never report one as an error.
- Never let a cache turn a "weigh this" instruction into a hard filter. `select` in particular must keep
  choosing among candidates rather than mechanically taking the cache's first row.
- An entry can be lost — a compaction can drop an append that lands in the wrong instant. The cost of
  losing one is re-doing one probe, which is why nothing may depend on an entry being present.

## One line per record

Append-only JSONL, one JSON object per line, and both halves of that matter for parallel safety. Eight
concurrent writers appending ~90-byte lines produce no interleaving; the same writers appending a
multi-line pretty-printed record corrupt the file beyond parsing, and appending 64KB single lines tears
384 of 400 lines. So:

- **One record is exactly one line.** Never pretty-print into a cache.
- **Keep a line under ~2KB**, and never over 4KB. Long prose does not belong in a cache — point at the
  run whose JSON holds it (`"run": "run-BF0WPt"`) and let the reader open that file.

Appending is how you write. No lock, no read-modify-write:

```sh
printf '%s\n' "$RECORD" >> /root/asterinas/.fix-conformance/select.cache
```

This is the same reason nothing else in this skill needs a mutex: writers only ever append, so there is
nothing to contend over.

## Read a snapshot, not the live file

Reading the live file while another run appends returns a torn line often enough to matter. Copy first,
then parse — 40 reads of a snapshot while four writers appended returned zero torn reads:

```sh
cp /root/asterinas/.fix-conformance/select.cache /tmp/sel.snap 2>/dev/null || : > /tmp/sel.snap
jq -s 'group_by(.suite + " " + .test + " " + .kind) | map(last)' /tmp/sel.snap
```

The `|| :` keeps a missing cache from failing the read. `group_by ... map(last)` is how you resolve
duplicates: the newest record for a given key wins, and records of different `kind` for the same test
stay independent. Drop lines that fail to parse rather than aborting the read.

## Every record carries the commit it was observed on

```json
{"kind":"probe","suite":"ltp","test":"dup03","result":"fail","commit":"d07835db2","run":"run-BF0WPt"}
```

An observation of kernel behavior is only meaningful against a kernel. `commit` is the run's
`base_commit`, and it is what tells you whether an entry still applies:

- **Same commit as your `base_commit`** — usable as-is.
- **An ancestor of yours** — a `pass` is still a strong hint, but a **`fail` must be re-probed**: the
  intervening commits may have fixed it. This is not hypothetical — the `dup03` fix turned `dup06` and
  `pipe06` green as a side effect, so their cached `fail` went stale the moment it landed.
- **Not an ancestor** — a parallel branch. Treat as a weak hint only.

```sh
git -C /root/asterinas merge-base --is-ancestor <cached-commit> <your-base-commit> && echo ancestor
```

Host-side observations are the exception: a `host` record describes the prebuilt binary and real Linux,
so it does not go stale when Asterinas changes. It stays valid until the suite's pinned version changes,
which is why a `host` record carries the version it was taken against.

## Size

Roughly 1700 candidates exist across the four suites (ltp ~975 commented plus 72 blocked, gvisor 563
cases, kselftest 99, xfstests 6), and a record runs ~150 bytes. Full coverage is therefore a few hundred
KB, and a few observations per candidate still lands under a megabyte. No pruning is needed at that
scale.

If a cache does pass ~2MB or ~5000 lines, compact it — reduce to the newest record per key, write a new
file, and `mv` it over the old one. `mv` is atomic, so a concurrent reader sees either the old file or
the new one, never a partial. Compaction is optional maintenance; never a prerequisite for using a
cache.

```sh
cd /root/asterinas/.fix-conformance
jq -cs 'group_by(.kind + " " + .suite + " " + .test) | map(last) | .[]' select.cache > select.cache.new \
  && mv select.cache.new select.cache
```

## What does not go in a cache

- **Judgements, as opposed to observations.** "dup03 failed with EMFILE on d07835db2" is an observation
  any run would reproduce. "dup03 is the best candidate" is one run's reasoning, and caching it would
  make the next run inherit that reasoning instead of doing its own — which matters because two runs on
  the same test exist precisely to compare how they reason. Cache what was measured, not what was
  concluded from it.
- **Anything long.** Root causes, invariants, and rationales live in the run's own JSON. A cache holds
  the key, the outcome, and the `run` pointer to the detail.
- **Anything a phase file already tells you to read off disk.** `state.json` and the per-run phase JSON
  are not cache material; they are already the interface.
