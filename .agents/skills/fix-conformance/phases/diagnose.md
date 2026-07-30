# Phase 2: diagnose

Establish what the real kernel does, watch Asterinas do something else, and explain why.

**In:** `select.json`; `diagnose.cache`, per [cache.md](../cache.md); `suites.md`. Run every command from
the worktree in `state.json`.
**Out:** `diagnose.json` in the run directory, and one appended line in `diagnose.cache`.

## Check `diagnose.cache` first

One line tells you whether another run already reached a verdict on this test, which is worth reading
before you spend a host run and a boot re-deriving it. Snapshot and reduce per
[cache.md](../cache.md):

```sh
cp /root/asterinas/.fix-conformance/diagnose.cache /tmp/dg.snap 2>/dev/null || : > /tmp/dg.snap
jq -s 'group_by(.suite + " " + .test) | map(last)' /tmp/dg.snap
```

A hit is a lead, not a conclusion — the entry holds the verdict and a `run` pointer, and the reasoning
behind it lives in that run's `diagnose.json`. Read that file before leaning on it:

- `missing-feature` or `no-bug` on your test, at your commit or an ancestor — the most valuable hit
  there is, since it can retire a run before it builds anything. Confirm the reasoning still holds, then
  reach that verdict yourself if it does. If you disagree, say why in `root_cause` and diagnose normally.
- `bug` — someone diagnosed this and did not land a fix, or landed one elsewhere. Their `invariant` and
  `owning_layer` are a strong starting hypothesis, and you still owe your own reproduction.
- `already-green` at an **ancestor** commit — re-verify rather than trust; a pool entry that was stale
  then is almost certainly stale now, but "almost certainly" is not evidence.
- Any verdict whose commit is **not** an ancestor of yours — a parallel branch, so a weak hint only.

A miss is the normal case. Diagnose from scratch and leave the entry behind for the next run.

## Establish the baseline

Every suite ships prebuilt binaries that run on **both** host Linux and Asterinas, and this
container *is* Linux. So the baseline is not guesswork: run the test's own binary on the host
(`suites.md` gives the path per suite) and read its output. When `select.json` sets `gvisor_filter`,
baseline exactly that scope — `<binary> --gtest_filter=<gvisor_filter>`, the same string — so the
comparison matches what this run will verify. Read the test source too: the binary tells you it passes,
the source tells you what it asserted.

The baseline is not "it passes on Linux". It is: for each assertion, what the correct kernel
returns.

For anything ambiguous, read the **specification** rather than inferring it from one observation: the
man page and, once you reach a verdict, Linux's own implementation of the call. Both are web reads —
this container has no kernel source and its man pages were stripped, so `man 2 <call>` returns only a
"system has been minimized" notice, not documentation.

## Go red in Asterinas

Run the test through the selector, which ignores the blocklist — that is what lets a
still-blocked test run at all:

```sh
PATH=<osdk_bin>:$PATH VNC_PORT=<vnc_port> make run_kernel AUTO_TEST=conformance \
    CONFORMANCE_TEST_SUITE=<suite> CONFORMANCE_TEST_SELECTOR=<selector_token> <run_vars> \
    CARGO_OSDK=<osdk_bin>/cargo-osdk
```

Both `selector_token` and `run_vars` come from `select.json` — use them verbatim. For a filtered gvisor
run, `run_vars` already carries `CONFORMANCE_TEST_GVISOR_FILTER=<gvisor_filter>`, so the boot runs that
scope and nothing else.

`osdk_bin` and `vnc_port` come from `state.json`. All three of `PATH`, `CARGO_OSDK=`, and `VNC_PORT`
are required; without the first two the build fails on a cargo lockfile collision that has nothing to
do with the test.

Read `qemu.log` for the per-case lines and diff against the baseline. If the test passes, the
verdict is `already-green` — write the JSON and stop, the pool entry is stale.

## Reach a verdict

Read the kernel code on the failing path and classify:

- **bug** — a localized defect in code that exists.
- **missing-feature** — needs a subsystem or syscall Asterinas does not implement.
- **no-bug** — Asterinas is right; the test or the environment is at fault.
- **already-green** — it passes; only the pool entry is wrong.

## Locate the invariant

For a `bug` verdict, finish by answering **which layer owns the invariant that was violated** — the
single most useful thing you hand to **fix**, and something only this phase has read enough code to
decide.

Read how Linux implements the same call (web) and note *where* it enforces the rule: in the syscall
entry, the VFS layer, the filesystem, the socket layer. That placement is the reference answer, and it
is evidence, not decoration — a rule Linux enforces in VFS is one Asterinas should generally also
enforce above the filesystem, or the same defect reappears through every other caller.

Where Asterinas's abstractions do not line up with Linux's, say so explicitly and name the layer that
is the closest equivalent. Asterinas is not obliged to copy Linux's structure; the fix is expected to
fit Asterinas's own design, and this is the field where that design decision gets recorded.

Name the layer even when the narrowest possible edit sits somewhere else — especially then. A patch at
the wrong layer can pass the target test and leave the same bug reachable through every other path,
which is the failure mode this field exists to prevent.

## Out shape

```json
{
  "verdict": "bug",
  "baseline": "what the correct kernel returns, per assertion",
  "failing_assertion": "the exact failing line and Asterinas's actual wrong value",
  "impl": { "file": "kernel/src/...", "lines": "1396-1423" },
  "root_cause": "the mechanism by which that code produces that wrong value",
  "invariant": "the rule that must hold, stated independently of this test",
  "owning_layer": "the layer that must enforce it, e.g. VFS above the filesystem",
  "linux_reference": "where Linux enforces the same rule, with the file/function read",
  "other_callers": "who else reaches this path and is affected by the same defect",
  "suggested_direction": "the correct fix at that layer"
}
```

`invariant` must be stated **without reference to the test** — "truncate beyond EOF zero-fills the
gap", not "truncate02 expects zeros". A rule that can only be phrased in terms of the test is a sign the
verdict is really a special case, and it is what makes the next phase able to check its own work.

`other_callers` is what turns the layer choice into something checkable: name who else reaches the
defect. When the honest answer is "nobody else", say that — it makes a narrow fix defensible instead of
merely convenient.

## Write your verdict to `diagnose.cache`

One appended line, after `diagnose.json` is written. Follow [cache.md](../cache.md) — one line, nothing
over ~2KB, no pretty-printing:

```sh
printf '%s\n' "$REC" >> /root/asterinas/.fix-conformance/diagnose.cache
```

```json
{"suite":"ltp","test":"dup03","verdict":"bug","commit":"d07835db2","run":"run-BF0WPt","owning_layer":"fd allocator in kernel/src/fs/file/file_table.rs","summary":"fd allocation never consults RLIMIT_NOFILE, so EMFILE is unreachable"}
```

`owning_layer` and `summary` are one short line each — a lead, not the analysis. Keep the full
`invariant`, `root_cause`, and `other_callers` in `diagnose.json`, which the `run` pointer reaches; a
verdict is worth caching, a paragraph is not. For a `missing-feature` or `no-bug` verdict `owning_layer`
may be null, and `summary` should name what is missing or why Asterinas is right — that one line is what
lets a later run retire before it builds anything.

Write the line for **every** verdict, including `already-green`. Also for a filtered gvisor run, where
`test` is the binary and `"case"` names the gtest case, so cases stay independently addressable.

Done when: `diagnose.json` exists and, for verdict `bug`, `root_cause` names the file, the lines, and the
mechanism — not a guess about which area is at fault — and `invariant` / `owning_layer` /
`linux_reference` are filled in.
