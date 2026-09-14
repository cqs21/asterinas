# Phase 5: commit

Land the kernel fix and the enabled test as one commit.

**In:** `select.json`, `diagnose.json`, `verify.json`, and `fix.json` — which is **absent** when the
verdict was `already-green`, since no fix round ran. Run every command from the worktree in
`state.json`.
**Out:** `commit.json` and the final, verified `patch.diff` in the run directory.

## Do

Run `make format`, then `make check` (rustfmt, clippy, typos, license headers). Both build, so both
need the make prefix:

```sh
PATH="<osdk_bin>:$PATH" VNC_PORT=<vnc_port> make check CARGO_OSDK="<osdk_bin>/cargo-osdk"
```

Fix what they report; a lint fix does not spend a **fix** round.

Stage every intended path from `fix.json` plus the pool edit (including intended new files and mode
changes) — name those paths explicitly, so scratch files never slip in. For an `already-green` run,
stage only the pool edit. Confirm with `git status` that nothing else is staged and no repro file survives in
`test/initramfs/src/regression/`.

For an `already-green` run there is no kernel change: the pool edit is the whole commit. Stage only
that, and write the message as described under [already-green](#already-green) below rather than the
rules for a fix.

Commit both **as one commit**: repo convention is that a fix and the conformance entry it
enables land together. Follow the log's style:

- Subject: one line, imperative, identifiers in backticks, describing the kernel change —
  e.g. ``Zero-fill extended regions in `exfat` truncate``. Not the test name.
- Body: the root cause, the fix, and the test now enabled. State the invariant and the layer that
  enforces it, from `diagnose.json`'s `invariant` / `owning_layer` and `fix.json`'s `layer` — a reviewer
  judges whether the fix belongs there, and cannot do that if the message only describes the edit.
  Include the focused validation that supports the fix beyond the target assertion when relevant.
  Where `linux_reference` explains *why* that is correct, say what Linux does; cite it as the reason the
  behavior is right, never as the justification on its own. When `gvisor_filter` is set, name the cases
  enabled and say the binary's others stay blocked — a reviewer seeing a gvisor blocklist lose 2 of 5
  lines needs the commit to say that was the intent.

### already-green

When `fix.json` is absent, both rules above invert — there is nothing to describe but the blocklist
itself, and a subject in the imperative about a kernel change would be false:

- Subject: name the enabling, e.g. ``Enable `truncate02` in the `exfat` conformance blocklist``.
- Body: say the test passes as-is and that the entry was stale, with `verify.json`'s evidence that it
  passes under the blocked configuration. Do not invent a root cause; there is no defect here.

Do not push. Do not open a PR. Do not amend anything that is not this commit.

## Archive the complete change

After committing, replace the fix-phase checkpoint with a patch from `base_commit` to the final commit.
This must include the pool edit, formatting/lint fixes, and new files, including for `already-green` runs.
Plain `git diff` after committing is empty and must not overwrite the patch.

Resolve `base_commit` and `commit` to full object IDs and require the final commit's sole parent to be
`base_commit`: this run delivers one commit. On resume, if that commit already exists, finish exporting
it rather than creating a second commit or repeating successful phases.

Use a temporary index to prove the archived patch reconstructs the exact committed tree from the base,
without changing the worktree or its index. In this example, `RUN_DIR` is the absolute run directory
and `BASE` / `COMMIT` are the full object IDs just checked:

```sh
(
    set -eu
    ARCHIVE_TMP=$(mktemp -d "$RUN_DIR/archive-XXXXXX")
    trap 'rm -rf "$ARCHIVE_TMP"' EXIT
    git diff --binary --full-index --no-ext-diff --no-textconv "$BASE" "$COMMIT" > "$ARCHIVE_TMP/patch.diff"
    test -s "$ARCHIVE_TMP/patch.diff"
    export GIT_INDEX_FILE="$ARCHIVE_TMP/index"
    git read-tree "$BASE"
    git apply --cached --check "$ARCHIVE_TMP/patch.diff"
    git apply --cached "$ARCHIVE_TMP/patch.diff"
    test "$(git write-tree)" = "$(git rev-parse "$COMMIT^{tree}")"
    mv "$ARCHIVE_TMP/patch.diff" "$RUN_DIR/patch.diff"
)
```

Only after this succeeds, write `commit.json` atomically with `patch_verified: true` and the final
`tree` object ID. Keep the previous checkpoint and the worktree on failure.
The orchestrator removes the worktree and branch after reading this result;
this archive must remain usable even after the unreferenced commit is garbage-collected.

## Out shape

```json
{
  "commit": "<full sha>",
  "base_commit": "<full base sha>",
  "tree": "<committed tree sha>",
  "patch": "<absolute run directory>/patch.diff",
  "patch_verified": true,
  "branch": "fix-ltp-truncate02-a3f9c1",
  "worktree": "<absolute path>",
  "subject": "the commit subject",
  "message": "the full commit message from git log -1 --format=%B",
  "files": ["kernel/src/...", "test/initramfs/src/conformance/..."]
}
```

Done when: `make check` passes, `git log -1` shows the commit containing both the kernel change and the
pool edit — or the pool edit alone, for `already-green` — `git status` in the worktree is clean,
and the final `patch.diff` reconstructs the committed tree from `base_commit`.
Keep the worktree and branch for orchestrator cleanup; do not remove them in this phase.
