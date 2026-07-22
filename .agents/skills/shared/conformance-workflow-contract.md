# Conformance-fix workflow — shared contract

The single source of truth for the file layout, JSON schemas, and worktree/git
rules shared by the five conformance-fix skills:

- `select-conformance-test` — pick a not-yet-passing test.
- `diagnose-and-fix` — compare against Linux, decide a verdict, optionally fix.
- `regress-and-verify` — prove the fix, guard against regressions.
- `package-patch` — build the final patch + PR description.
- `fix-conformance` — the orchestrator that runs the four in sequence.

Every skill reads this file first. Do not re-describe these rules inside a skill;
link here.

---

## 0. Vocabulary

The canonical terms these skills use. Use them verbatim in outputs, test names, and PR
text. (The repo may also keep a root `CONTEXT.md` glossary; this section is the
self-contained copy the skills depend on, so they work even where `CONTEXT.md` is
absent.)

- **Conformance suite** — one of the four external test suites Asterinas runs to measure
  Linux compatibility: `gvisor`, `ltp`, `kselftest`, `xfstests`. Run in QEMU via
  `make run_kernel AUTO_TEST=conformance CONFORMANCE_TEST_SUITE=<suite>`.
- **Blocklist** — the set of cases a suite is known to fail, under
  `test/initramfs/src/conformance/<suite>/`. A "not-yet-passing test" is a blocklisted
  case. For LTP the blocklist applies at *packaging* time (the binary isn't placed in
  the initramfs); for gvisor/kselftest/xfstests it is a runtime filter.
- **Selector** (`CONFORMANCE_TEST_SELECTOR`) — a knob (PR #3598, assumed available) to run
  exactly one/a few named tests within a suite, ignoring the blocklist. Comma-separated;
  per-suite token format — gvisor: binary name; kselftest: `collection:case`; ltp:
  testcase id; xfstests: test id. `CONFORMANCE_TEST_GVISOR_FILTER` narrows gtest cases
  inside one gvisor binary.
- **Enable a conformance test** — remove its entry from the suite's blocklist so it runs
  in the default (blocklist-subtracted) suite run again.
- **Regression test** — an Asterinas-authored minimal-reproduction C program under
  `test/initramfs/src/regression/`, distinct from the external suites. Delivered
  separately; **never** inside the final patch.
- **Verdict** — the diagnose stage's conclusion: `bug` (a fixable defect), `no-bug`
  (Asterinas already correct; test/env at fault), `missing-feature` (needs a large
  implementation beyond a minimal fix).
- **Fix diff** — `patch.diff` containing only the kernel code fix, emitted by
  `diagnose-and-fix`. Standalone, the complete deliverable.
- **Final patch** — the orchestrated deliverable: kernel fix + blocklist removal only.
- **Work directory** vs **problem worktree** — see §1/§4.

---

## 1. Directories

Two distinct roots, kept separate on purpose:

- **Work directory** (`<work-dir>`) — a user-specified, **persistent** path (not under
  version control). Holds all artifacts and the resume state. One **problem
  sub-directory** per problem:

  ```
  <work-dir>/
  ├── run.json                       # orchestrator-level: N, suite, attempted tests
  └── <problem-id>/                  # e.g. gvisor__epoll_test
      ├── state.json                 # which stages are done; drives resume
      ├── select.json
      ├── diagnose.json
      ├── root-cause.md              # human-readable twin of diagnose.json
      ├── verify.json
      ├── patch.diff                 # final patch (fix + blocklist removal)
      ├── PR.md                      # PR description
      └── regression/                # copy of the regression test (delivered separately)
  ```

  `<problem-id>` = `<suite>__<test>` with every character outside `[A-Za-z0-9._-]`
  replaced by `_` (so `timers:posix_timers` → `timers_posix_timers`, `generic/001`
  → `generic_001`).

- **Problem worktree** (`<worktree>`) — a **temporary** git worktree branched from
  `main`, where kernel code, the regression test, and the blocklist edit are made,
  and where the kernel is built/run. Deleted only after the problem completes *and*
  its outputs are persisted to `<work-dir>`. See §4.

The fix diff and final patch are extracted from `<worktree>` into `<work-dir>`
**before** the worktree is removed.

---

## 2. Stage JSON schemas

All JSON files are UTF-8, 2-space indented. Unknown fields are ignored by readers,
so a stage may add fields without breaking others. A skill validates that the input
JSON it consumes has the required fields and fails loudly if not.

### `select.json` (written by `select-conformance-test`)

```json
{
  "suite": "gvisor",
  "test": "epoll_test",
  "gvisor_filter": "EpollTest.CloseFile:EpollTest.Oneshot",
  "blocklist_file": "test/initramfs/src/conformance/gvisor/blocklists/epoll_test",
  "blocklist_entry": "EpollTest.EpollPwait2Timeout",
  "source": "auto-picked",
  "difficulty": "easy",
  "rationale": "Blocklist comment attributes failure to jiffies-resolution timeout; localized.",
  "linked_refs": ["https://github.com/asterinas/asterinas/pull/1035"]
}
```

- `suite` — `gvisor` | `ltp` | `kselftest` | `xfstests`.
- `test` — the selector token in that suite's format (gvisor: binary name;
  kselftest: `collection:case`; ltp: testcase id; xfstests: test id).
- `gvisor_filter` — optional, gvisor only; positive gtest filter narrowing cases
  inside the binary. Empty/absent otherwise.
- `blocklist_file` — repo-relative path of the blocklist file holding the entry
  (for gvisor/kselftest the per-binary file; for ltp the `blocked/*.txt`; for
  xfstests `block.list`). May be null if the test is not currently blocklisted.
- `blocklist_entry` — the exact line to remove to enable the test. May be null.
- `source` — `user-specified` | `auto-picked`.
- `difficulty` — `easy` | `medium` | `hard` (auto-pick prefers `easy`).
- `rationale` — why this test, and why this difficulty (cite the blocklist comment).
- `linked_refs` — URLs/issue ids found in the blocklist comment, if any.

### `diagnose.json` (written by `diagnose-and-fix`)

```json
{
  "verdict": "bug",
  "linux_reference": {
    "source": "man7.org/linux/man-pages/man2/epoll_pwait2.2.html",
    "kind": "man-page",
    "semantics": "epoll_pwait2 must not return before the requested timeout elapses."
  },
  "asterinas_impl": {
    "file": "kernel/src/syscall/epoll.rs",
    "lines": "120-155",
    "status": "present-buggy"
  },
  "root_cause": "Prose root-cause; mirrored verbatim to root-cause.md.",
  "suggested_direction": "Use a monotonic-clock deadline instead of a jiffies counter.",
  "fix_applied": true,
  "fix_diff": "patch.diff",
  "changed_files": ["kernel/src/syscall/epoll.rs"]
}
```

- `verdict` — `bug` (a fixable defect) | `no-bug` (Asterinas already correct; the
  test or environment is at fault) | `missing-feature` (needs a large implementation
  beyond a minimal fix).
- `linux_reference.kind` — `man-page` | `linux-source` | `gvisor-test` | `posix` | `other`.
- `linux_reference.source` — where the semantics came from; note `local` if a local
  tree was used, else the URL/path fetched.
- `asterinas_impl.status` — `present-buggy` | `absent` | `correct`.
- `suggested_direction` — always populated, even when no fix is applied, so a
  report-only caller gets a concrete next step.
- `fix_applied` — true iff code was changed in the worktree.
- `fix_diff` — path (relative to the problem sub-dir) of the emitted fix diff, or
  null. This is the **fix-only** diff (no blocklist edit). Standalone, this is the
  deliverable.
- `changed_files` — repo-relative paths touched by the fix.

### `verify.json` (written by `regress-and-verify`)

```json
{
  "regression_test": { "path": "test/initramfs/src/regression/fs/epoll_timeout/", "passed": true },
  "selected_test": { "passed": true },
  "full_suite": { "suite": "gvisor", "passed": true, "regressions": [], "newly_passing": ["epoll_test"] },
  "status": "ok"
}
```

- `status` — `ok` | `selected-failed` (fix did not make the selected test pass) |
  `regressed` (a previously-passing test now fails; `full_suite.regressions` lists
  them). Anything other than `ok` sends the orchestrator back to `diagnose-and-fix`
  (bounded retry, see §5).

### `state.json` (written/updated by the orchestrator; readable by all)

```json
{
  "problem_id": "gvisor__epoll_test",
  "suite": "gvisor",
  "test": "epoll_test",
  "worktree": "/root/asterinas/.worktrees/gvisor__epoll_test",
  "stages": {
    "select": "done",
    "diagnose-fix": "done",
    "regress-verify": "in-progress",
    "package": "pending"
  },
  "attempts": 1,
  "outcome": null
}
```

- `stages.<name>` — `pending` | `in-progress` | `done`.
- `attempts` — diagnose→verify retry count for this problem (see §5).
- `outcome` — null until the problem finishes; then `packaged` | `abandoned-no-bug`
  | `abandoned-missing-feature` | `gave-up` (retries exhausted).

### `run.json` (orchestrator top-level)

```json
{
  "suite": "gvisor",
  "target_count": 1,
  "attempted_tests": ["epoll_test"],
  "completed_problems": ["gvisor__epoll_test"]
}
```

`attempted_tests` feeds the next `select` call's `exclude` so the loop never
re-picks a test it already tried (fixed, abandoned, or gave up).

---

## 3. Spawning stages in isolated subagents

Each stage runs in its own subagent with a clean context. The orchestrator passes
**only** the problem sub-dir path and the worktree path — never its own conversation
context. The spawn primitive is the only agent-specific part:

- **Claude Code** — spawn a `Task` sub-agent; its prompt says "invoke the
  `<skill>` skill with these inputs" and names the JSON paths.
- **Codex** — `codex exec "<prompt>"` with the same prompt text.

A stage's subagent reads its input JSON from disk, does its work, writes its output
JSON (and any Markdown/artifacts) to disk, and returns a one-line status. The
orchestrator reads the output **from disk**, not from the subagent's return text.

---

## 4. Worktree & git rules

- Branch from `main`: `git worktree add <worktree> -b conformance/<problem-id> main`.
  Place worktrees under `<repo>/.worktrees/` (git-ignored) unless the user gives a
  path.
- All kernel edits, the regression test, and the blocklist removal happen inside
  `<worktree>`. The user's current branch/working tree is never touched.
- **Fix diff** = `git -C <worktree> diff -- <changed kernel files>` (excludes the
  regression test and the blocklist edit).
- **Final patch** = `git -C <worktree> diff` restricted to the kernel fix **and** the
  blocklist file — never the regression test under `test/initramfs/src/regression/`.
- Delete the worktree (`git worktree remove`) **only** after the problem's outcome is
  set and all artifacts are copied into `<work-dir>`. If a run is interrupted
  (process/network failure) leave the worktree in place so the problem can resume.
- Never commit, push, or open a PR. These skills produce a `patch.diff` and a `PR.md`
  file; a human decides what to do with them.

---

## 5. Retry & completion rules

- **Verify failure** (`selected-failed` or `regressed`): the orchestrator re-enters
  `diagnose-and-fix` in `auto-fix` mode, passing `verify.json` so the fixer sees what
  failed. Bounded at **2 attempts** total per problem. On exhaustion set
  `outcome = "gave-up"`, keep the worktree and artifacts, and report.
- **A problem's flow is complete when** any of:
  1. `package-patch` has written `patch.diff` + `PR.md` (`outcome = "packaged"`).
  2. `diagnose-and-fix` returns `no-bug`/`missing-feature` and the user did not ask to
     continue (`outcome = "abandoned-no-bug"` / `"abandoned-missing-feature"`). For an
     **auto-picked** test the orchestrator moves to the next candidate; for a
     **user-specified** test it stops and reports.
  3. Standalone `diagnose-and-fix` has emitted `patch.diff` (fix diff) + `root-cause.md`.
- **Verdict gating**: `bug` → fix (auto in orchestrated mode; in standalone
  `report-only` mode, report and wait). `no-bug`/`missing-feature` → do **not** fix,
  **unless** the caller passed a `continue_direction` for a user-specified test, in
  which case attempt the fix along that direction.

---

## 6. Running things (inside the project Docker container)

Conformance and kernel runs happen in QEMU via the Asterinas Docker container (see
`AGENTS.md`). Key commands, all run from the worktree root:

```sh
# Run one selected test (ignores the blocklist; PR #3598).
make run_kernel AUTO_TEST=conformance \
    CONFORMANCE_TEST_SUITE=<suite> CONFORMANCE_TEST_SELECTOR=<test> \
    [CONFORMANCE_TEST_GVISOR_FILTER=<filter>]

# Full suite run (blocklist applied) — the no-regression check.
make run_kernel AUTO_TEST=conformance CONFORMANCE_TEST_SUITE=<suite>

# Regression tests.
make run_kernel AUTO_TEST=regression
```

Runners print each executed case and a passed/total summary; parse that text to
decide pass/fail. There is no unit-test harness for these paths.
