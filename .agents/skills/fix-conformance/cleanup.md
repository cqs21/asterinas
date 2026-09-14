# Finish and clean up a run

The orchestrator performs this after a terminal outcome, for both `committed` and `abandoned` runs.
Keep `<repo_root>/.fix-conformance/<run-id>/`, including the JSON, final `patch.diff`, and any evidence
referenced by the final report. Remove the run's worktree, per-run OSDK/build output, and branch.
Never clean an `in-progress` run: those resources are its resume environment.
Load `REPO_ROOT` from this run's `state.json.repo_root` after the setup/resume path checks.
Do not infer the root from the worktree being removed or from the process's current directory.

## Before deleting resources

1. Require a durable terminal result and no phase agent or run-owned build/QEMU process still using the worktree.
   For a committed run, all phases must be done, including an explicitly skipped fix phase.
2. Require a full `base_commit` and a verified final archive for a committed run:
   `commit.json` names the base, commit, tree, patch path, and `patch_verified: true`,
   and the nonempty patch still reconstructs that tree using the temporary-index check in
   [commit.md](phases/commit.md#archive-the-complete-change).
   For an already archived result, compare `git write-tree` with the saved `tree` ID;
   the original commit object is not required.
   Merely having a `patch.diff` from fix is insufficient: it may omit the pool edit.
   If a legacy run lacks these fields, archive and check its final commit by that procedure first,
   without rerunning tests or creating a new commit.
   If the final commit is unavailable and the patch cannot be verified, report the limitation and preserve remaining resources.
3. For an abandoned run, retain its verdict and attempts. Preserve any remaining intended changes as
   an explicitly unverified checkpoint before deletion; do not replace an existing failed-attempt patch
   with an empty diff after fix has reset the worktree.
4. Check ownership using the exact paths and branch saved in `state.json` and
   `git -C "$REPO_ROOT" worktree list --porcelain`.
   Require the worktree to be this run's `<repo_root>/.worktrees/<run-id>` and the branch to be its
   recorded run-specific branch (or null if selection never created one).
   Check the current HEAD and tracked/non-ignored changes against the archived work.
   Preserve resources if they contain unrelated changes or the branch has advanced beyond the archived commit.
   Ignored build output and `.osdk/` are expected disposable contents.

Write `outcome` and `cleanup: "pending"` atomically before removing anything.
Record `cleanup_head` as the full archived commit ID, or the checked HEAD for an abandoned run,
so a resumed cleanup can detect a branch that moved even after the worktree is gone.
Keep the original `worktree`, `branch`, and `osdk_bin` fields as historical metadata.

## Remove only this run's resources

Set `cleanup` to `running`. From the main repository, remove the checked worktree first:

```sh
git -C "$REPO_ROOT" worktree remove --force "<recorded-worktree>"
```

`--force` discards this run's archived abandoned changes and generated files;
use it only after the ownership and preservation checks above.
Then delete the recorded branch, if non-null and still pointing at `cleanup_head`:

```sh
git -C "$REPO_ROOT" branch -D "<recorded-branch>"
```

Do not remove another worktree to free a branch checked out elsewhere.
Do not use a wildcard, repository-wide prune, or broad process kill as a substitute for these checks.
Confirm both the worktree registration/path and branch are absent, then atomically set `cleanup` to `done`.

Cleanup is idempotent: an already absent resource satisfies its removal step.
On resume, reuse the persisted `cleanup_head`; do not try to read HEAD from a removed worktree.
If interrupted between the two removals, `resume <run-id>` completes only the remaining cleanup.
If a check or removal fails, retain the terminal outcome, leave cleanup pending, and record `cleanup_error`.
Report the remaining resource and reason; do not claim it was removed or rerun completed phases.
