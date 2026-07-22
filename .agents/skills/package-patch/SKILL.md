---
name: package-patch
description: Compose the final deliverable for a fixed conformance test — a patch.diff containing the kernel fix plus the blocklist removal (the enabled conformance test) only, and a PR.md with root-cause analysis, solution, and verification evidence. The regression test is deliberately excluded. Use as the final stage of the fix-conformance workflow.
---

# package-patch

Turn a verified fix into the two hand-off artifacts: `patch.diff` (the **final patch**)
and `PR.md`. Runs only after `regress-and-verify` reports `ok`.

Read `../shared/conformance-workflow-contract.md` first (§4 what the final patch
contains, §2 schemas, §0 vocabulary).

## Inputs

- `select.json`, `diagnose.json`, `verify.json` (from the problem sub-directory).
- `worktree` (**required**) — holds the fix, the blocklist edit, and the regression
  test. The final patch is extracted from here **before** the worktree is removed.
- `out-dir` (optional) — problem sub-directory for outputs; defaults to cwd.

## 1. Compose the final patch

The final `patch.diff` contains **exactly**:

- the kernel fix (`diagnose.json.changed_files`), and
- the blocklist removal that enabled the conformance test
  (`select.json.blocklist_file`).

It must **not** contain the regression test under
`test/initramfs/src/regression/`. Build it by restricting the diff to just those paths:

```sh
git -C <worktree> diff -- <changed_files...> <blocklist_file> > <out-dir>/patch.diff
```

Then verify the exclusion: confirm `patch.diff` mentions the kernel files and the
blocklist file, and contains **no** `test/initramfs/src/regression/` path. If a
regression path leaks in, the path restriction was wrong — fix it and regenerate. (The
regression test has already been copied to `<work-dir>/<problem-id>/regression/` as the
separate deliverable.)

## 2. Write PR.md

A reviewer-facing description. Wrap identifiers in backticks; keep the title concise
(≤70 chars). Structure:

- **Title** — e.g. ``Fix `epoll_pwait2` timeout resolution`` (imperative, specific).
- **Motivation / symptom** — which conformance test was failing and how.
- **Root-cause analysis** — from `root-cause.md` / `diagnose.json`: the Linux/POSIX
  contract (cite the source in `linux_reference`), what Asterinas did instead, and the
  precise cause.
- **Solution** — what the fix changes and why it's minimal and correct; note the
  `kernel/` safe-Rust constraint held.
- **Conformance impact** — the blocklist entry removed (the enabled test), and the
  verification evidence from `verify.json`: selected test now passes; full `<suite>`
  run shows no regressions (`newly_passing`, empty `regressions`).
- **Testing** — note that a separate minimal regression test
  (`verify.json.regression_test.path`) reproduces the bug and is delivered alongside
  but intentionally kept out of this patch.

## Output

Write `patch.diff` and `PR.md` into the problem sub-directory. Report the two paths and
a one-line summary. The orchestrator sets `outcome = "packaged"`, persists artifacts,
and only then removes the worktree.

Never commit, push, or open a PR — produce the files and let a human decide.
