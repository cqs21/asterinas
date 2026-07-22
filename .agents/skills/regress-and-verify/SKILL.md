---
name: regress-and-verify
description: Prove a kernel fix with a minimal-reproduction regression test, then enable the fixed conformance test and confirm the affected suite still fully passes (no regressions). Use as the third stage of the fix-conformance workflow, or standalone against an already-fixed worktree.
---

# regress-and-verify

With a fix already applied in the worktree, do three things: write and pass a
**minimal-reproduction regression test**, confirm the **selected conformance test now
passes**, and run the **affected suite's full run** to prove no previously-passing test
regressed. Write `verify.json`.

Read `../shared/conformance-workflow-contract.md` first (§2 `verify.json` schema, §4
worktree rules, §6 run commands, §0 vocabulary). All builds/runs
happen **inside the worktree**, in the project Docker container (see `AGENTS.md`).

## Inputs

- `select.json` — `suite`, `test`, `gvisor_filter`, `blocklist_file`, `blocklist_entry`.
- `diagnose.json` — `changed_files`, `root_cause` (informs what the regression test
  should exercise).
- `worktree` (**required**) — the worktree holding the applied fix.
- `verify.json` (rewritten each attempt).
- `out-dir` (optional) — problem sub-directory; defaults to cwd.

## 1. Write the regression test (minimal reproduction)

Asterinas regression tests are small C programs under
`test/initramfs/src/regression/<area>/<name>/`, built by the shared Makefile pattern
and run by a per-directory `run_test.sh`. Study a sibling (e.g.
`test/initramfs/src/regression/fs/getcwd/` and its `Makefile` that just
`include ../../common/Makefile`) before adding one.

1. Pick the area directory matching the syscall/subsystem (`fs`, `io`, `ipc`, `memory`,
   `network`, `process`, `time`, `device`, `security`).
2. Create `<area>/<name>/` with:
   - a `.c` file that reproduces **only** the specific bug — assert the Linux-correct
     behavior; it must **fail on the buggy kernel and pass on the fixed one**;
   - a `Makefile` (`include ../../common/Makefile`, plus `SUBDIRS`/filters only if
     needed — copy the sibling's shape);
   - wire it into the parent's run path: add the binary invocation to the enclosing
     area's `run_test.sh`, and the subdir to the parent `Makefile`'s `SUBDIRS` if it's
     a new leaf.
3. Keep it minimal and use `C_FLAGS += -Wall -Werror`-clean code (the common Makefile
   enforces this). Match the existing tests' idioms.

Build and run the regression suite in the worktree and confirm the new test passes:

```sh
make run_kernel AUTO_TEST=regression
```

Record `regression_test.path` and `regression_test.passed`. The regression test lives
in the worktree; the orchestrator copies it into `<work-dir>/<problem-id>/regression/`.
It is delivered **separately** and must **never** enter the final patch.

## 2. Enable and run the selected conformance test

1. Remove the test's entry from the blocklist so it becomes enabled (delete
   `blocklist_entry` from `blocklist_file`; for gvisor that's the one gtest case line,
   for ltp the `blocked/*.txt` line, for kselftest the `<dir>:<case>` line, for
   xfstests the `block.list` id). Leave surrounding comments coherent.
2. Confirm it passes, using the selector to run just it (ignores the blocklist anyway):

   ```sh
   make run_kernel AUTO_TEST=conformance \
       CONFORMANCE_TEST_SUITE=<suite> CONFORMANCE_TEST_SELECTOR=<test> \
       [CONFORMANCE_TEST_GVISOR_FILTER=<gvisor_filter>]
   ```

   Parse the runner's per-case / passed-total output. Set `selected_test.passed`. If it
   fails, set `status = "selected-failed"` and stop (the orchestrator retries via
   `diagnose-and-fix`).

## 3. No-regression check — affected suite, full run

Run the **whole selected suite** (blocklist applied, minus the entry you just removed).
Scope is this one suite only — not all four.

```sh
make run_kernel AUTO_TEST=conformance CONFORMANCE_TEST_SUITE=<suite>
```

Compare against the suite's expected baseline (every non-blocklisted case should still
pass, plus the newly-enabled one). Any case that previously passed and now fails is a
**regression**: list them in `full_suite.regressions` and set `status = "regressed"`.
If clean, `full_suite.passed = true`, `newly_passing` includes the enabled test,
`status = "ok"`.

## Output

Write `verify.json` (contract §2). Report a one-line status:
- `ok` — regression test passes, selected test passes, no regressions.
- `selected-failed` — fix didn't make the selected test pass.
- `regressed` — a previously-passing test now fails (listed).

Non-`ok` statuses send the orchestrator back to `diagnose-and-fix` (bounded retry, §5).
Do not delete the worktree; the orchestrator owns its lifecycle.
