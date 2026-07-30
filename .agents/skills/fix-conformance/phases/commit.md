# Phase 5: commit

Land the kernel fix and the enabled test as one commit.

**In:** `select.json`, `diagnose.json`, `verify.json`, and `fix.json` — which is **absent** when the
verdict was `already-green`, since no fix round ran. Run every command from the worktree in
`state.json`.
**Out:** `commit.json` in the run directory.

## Do

Run `make format`, then `make check` (rustfmt, clippy, typos, license headers). Both build, so both
need the make prefix:

```sh
PATH=<osdk_bin>:$PATH make check CARGO_OSDK=<osdk_bin>/cargo-osdk
```

Fix what they report; a lint fix does not spend a **fix** round.

Stage the kernel change and the pool edit — those paths specifically, so scratch files never
slip in. Confirm with `git status` that nothing else is staged and no repro file survives in
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

## Out shape

```json
{
  "commit": "<sha>",
  "branch": "fix-ltp-truncate02-a3f9c1",
  "worktree": "<absolute path>",
  "subject": "the commit subject",
  "files": ["kernel/src/...", "test/initramfs/src/conformance/..."]
}
```

Done when: `make check` passes, `git log -1` shows the commit containing both the kernel change and the
pool edit — or the pool edit alone, for `already-green` — and `git status` in the worktree is clean.
