# Caches

Knowledge one run paid for that the next should not pay for again.

`select` and `diagnose` each own one cache file, shared by every run:

```
<repo_root>/.fix-conformance/select.cache      # candidate observations
<repo_root>/.fix-conformance/diagnose.cache    # verdicts
```

`REPO_ROOT` comes from the run's `state.json.repo_root`, as described in [suites.md](suites.md).
They sit beside the run directories. Completed runs retain their JSON and final patch while their
worktrees and branches are removed, so a cache can still point to the supporting evidence.

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
printf '%s\n' "$RECORD" >> "$REPO_ROOT/.fix-conformance/select.cache"
```

This is the same reason nothing else in this skill needs a mutex: writers only ever append, so there is
nothing to contend over.

## Read a snapshot, not the live file

Reading the live file while another run appends returns a torn line often enough to matter. Copy first
into a snapshot unique to this phase, then parse — never use a shared filename under `/tmp`, because
parallel runs would overwrite one another's snapshots:

```sh
SEL_SNAP=$(mktemp "${TMPDIR:-/tmp}/fix-conformance-select.XXXXXX")
trap 'rm -f "$SEL_SNAP"' EXIT
cp "$REPO_ROOT/.fix-conformance/select.cache" "$SEL_SNAP" 2>/dev/null || : > "$SEL_SNAP"
jq -s 'group_by([.kind, .suite, .test, .case, .config, .version, .host_environment] | tojson) | map(last)' "$SEL_SNAP"
```

The `|| :` keeps a missing cache from failing the read. `group_by ... map(last)` is how you resolve
duplicates: the newest record for a given key wins, and records of different `kind` for the same test
stay independent. Drop lines that fail to parse rather than aborting the read.

## Match the configuration before reusing evidence

Every record includes `config`: the exact `run_vars` used for that candidate, or `""` for the default.
For gvisor also include `case` so different cases remain independent.
Reduce select records by `(kind, suite, test, case, config, version, host_environment)` and diagnose
records by `(suite, test, case, config)`. Missing optional fields are null, but missing `config` means
unknown, not the default configuration.
Do not merge observations across filesystems, filters, or extra blocklists.

A `host` record also includes the suite `version` and a compact `host_environment` object:
running kernel release, architecture, and the relevant execution conditions.
For filesystem-dependent tests, include actual test-file location, advertised filesystem type,
implementation/driver evidence, and relevant mount options; include credentials or namespaces where they affect assertions.
Keep detailed commands and evidence in the run's JSON and point to it with `run`.
Reuse an observation only when these conditions match the proposed baseline;
follow [linux-baseline.md](linux-baseline.md) to establish comparability.
A pass on tmpfs or ext4 is not native ext2 evidence, and host `tconf`/`tbroke` does not establish an Asterinas limitation.

Legacy records without configuration or sufficient driver evidence are discovery hints only.
Re-derive their Linux expectation from the matching source before using them to fix or enable a test.
This also applies to old `diagnose` verdicts whose Linux reference described a different backend.

## Asterinas observations carry the commit they were observed on

```json
{"kind":"probe","suite":"ltp","test":"dup03","config":"","result":"fail","commit":"d07835db2","run":"run-BF0WPt"}
```

An observation of kernel behavior is only meaningful against a kernel. `commit` is the run's
`base_commit`, and it is what tells you whether an entry still applies:

- **Same commit as your `base_commit`** — usable as-is.
- **An ancestor of yours** — a `pass` is still a strong hint, but a **`fail` must be re-probed**: the
  intervening commits may have fixed it. This is not hypothetical — the `dup03` fix turned `dup06` and
  `pipe06` green as a side effect, so their cached `fail` went stale the moment it landed.
- **Not an ancestor** — a parallel branch. Treat as a weak hint only.

```sh
git -C "$REPO_ROOT" merge-base --is-ancestor <cached-commit> <your-base-commit> && echo ancestor
```

Host-side observations are the exception: a `host` record describes the prebuilt binary and real Linux,
so it does not go stale when Asterinas changes.
It remains reusable only while the suite version and relevant host environment still match.
Asterinas commit ancestry cannot validate a host observation.

## Size

Roughly 1700 candidates exist across the four suites (ltp ~975 commented plus 72 blocked, gvisor 563
cases, kselftest 99, xfstests 6), and a record runs ~150 bytes. Full coverage is therefore a few hundred
KB, and a few observations per candidate still lands under a megabyte. No pruning is needed at that
scale.

If a cache does pass ~2MB or ~5000 lines, compact it — reduce to the newest record per key, write a new
file, and `mv` it over the old one. First confirm that no `select` phase is appending to this cache;
otherwise the replacement can race with an append and lose that record. `mv` is atomic, so a concurrent
reader sees either the old file or the new file, never a partial.
Compaction is optional maintenance; never a prerequisite for using a cache.

```sh
cd "$REPO_ROOT/.fix-conformance"
SEL_SNAP=$(mktemp "${TMPDIR:-/tmp}/fix-conformance-select.XXXXXX")
trap 'rm -f "$SEL_SNAP"' EXIT
cp select.cache "$SEL_SNAP"
jq -cs 'group_by([.kind, .suite, .test, .case, .config, .version, .host_environment] | tojson) | map(last) | .[]' "$SEL_SNAP" > select.cache.new \
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
