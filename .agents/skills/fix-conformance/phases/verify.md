# Phase 4: verify

Enable the test, then prove the whole suite still passes.

**In:** `select.json`, `diagnose.json`, `fix.json`; `suites.md`. Run every command from the
worktree in `state.json`.
**Out:** `verify.json` in the run directory.

## Enable the test

Apply `enable_action` from `select.json` to `pool_file`, touching **only** the lines in
`pool_entries`: uncomment them, delete them, or add the binary to `TESTS`. This edit is part of
the commit.

When `gvisor_filter` is set, `pool_entries` is a subset of that blocklist file — every other line in it
must survive untouched, since those cases stay blocked and the full-suite run below still expects them
to be skipped. Removing one line too many turns a clean verify into a suite failure.

Check the edit with `git diff` on `pool_file` **before** booting: the removed lines must equal
`pool_entries` exactly. This costs nothing, while the same mistake found afterwards costs a full-suite
run. Fix your own edit and re-check — this is not a fix-phase problem, and sending it back there would
chase a defect that is not in the kernel.

If `fix.json` is missing because the verdict was `already-green`, this edit is the entire change.

## Run the full suite

No selector — an empty selector is what makes the harness apply the blocklist, which is the
CI path and the thing being verified. Run the one configuration the entry was blocked under,
the `run_vars` from `select.json`:

```sh
PATH=<osdk_bin>:$PATH VNC_PORT=<vnc_port> make run_kernel AUTO_TEST=conformance \
    CONFORMANCE_TEST_SUITE=<suite> <run_vars> CARGO_OSDK=<osdk_bin>/cargo-osdk
```

That configuration only — the other CI variants are CI's job. `osdk_bin` and `vnc_port` come from
`state.json`; all three parts of the prefix are required.

Read `qemu.log` for the per-case results. Two things must hold: the target test appears in the
passing output, and no test that passed before now fails. When `gvisor_filter` is set, *each* line in
`pool_entries` must appear passing — and the binary's still-blocked cases must not appear at all, since
their presence means the blocklist edit took out more than it should. A run that fails only on the
target means the pool edit outran the fix; a run that fails on *other* tests is a **regression** —
report it and let the orchestrator send it back to **fix**.

## Out shape

```json
{
  "status": "ok",
  "target_test": { "name": "truncate02", "case_results": null, "passed": true },
  "still_blocked_absent": true,
  "full_suite": {
    "suite": "ltp",
    "config": "CONFORMANCE_TEST_WORKDIR=/exfat",
    "passed": true,
    "summary": "the passed/total line from qemu.log",
    "regressions": []
  }
}
```

`status` is `ok`, `regressed`, or `bad-pool-edit`. `regressions` lists each newly-failing test with what
`qemu.log` said about it. For a filtered gvisor run, make `target_test.case_results` a per-case pass map
so a partial result stays visible instead of collapsing into one boolean — it is `null` for suites whose
unit of work is the whole test. Set `still_blocked_absent` from the log check; it is `true` for any suite
with nothing left blocked in that file.

`bad-pool-edit` is for the case the pre-boot check exists to prevent: still-blocked cases showed up in
the run. Report it rather than sending a kernel regression that does not exist to **fix** — the kernel is
fine and the blocklist is not.

Done when: the full suite passed, every targeted test or case is in its passing output, nothing still
blocked appeared, and `regressions` is empty — or `status` is `regressed` and every regression is named.
