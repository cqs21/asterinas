# Phase 3: fix

Make the target test pass, in at most 5 rounds.

**In:** `select.json`, `diagnose.json` — especially its `invariant`, `owning_layer`, `linux_reference`,
and `other_callers`, which are what this phase is judged against; `suites.md`. If the orchestrator
passed a regression from **verify**, that regression is the failure to explain this round. If it passed
a user fix direction for a `no-bug`/`missing-feature` verdict, follow that direction.
**Out:** `fix.json` and `patch.diff` in the run directory. Run every command from the
worktree in `state.json`.

## Do

Fix the root cause at the layer `diagnose` named in `owning_layer`, so that `invariant` holds for every
caller — not only for the path this test walks. Among changes that do that, take the smallest: size is
the tiebreaker, not the goal.

Follow `linux_reference` for *what correct means*. Where Asterinas's abstractions do not match Linux's,
design against Asterinas's own abstractions rather than transplanting Linux's structure, and record in
`fix_summary` which abstraction you chose and why.

**A special case is not a fix.** Before writing `patch.diff`, check your own change: is it still correct
for the callers in `other_callers`, and for inputs this test never supplies? A branch on the exact flag,
size, or errno the test happens to use is overfitted to the test. Green is necessary but not sufficient —
this commit has to survive review as a fix to the kernel, not to the test.

Re-run the selector command from **diagnose** — with the same make prefix (`PATH=<osdk_bin>:$PATH`,
`VNC_PORT=`, `CARGO_OSDK=`) — and read `qemu.log`.

If a minimal C repro sharpens the loop, keep it in `/tmp` and run it on the host for its
baseline. It is scratch: never committed, and never added to
`test/initramfs/src/regression/`.

After **every** round — green or not — write `patch.diff` with `git diff` in the worktree, so
an interruption never loses the work.

## The budget

5 rounds. Each round must start from a **new hypothesis** read out of the failure output, not
a tweak of the last patch. If the same hypothesis fails twice, widen the read before patching
again: the callers, the layer above (VFS, the socket layer), a second syscall on the path.

Two failures at one layer are also evidence about the **layer itself**. Re-read
`diagnose.json`'s `owning_layer` and Linux's placement of the same rule before spending a third round
below it — patching further and further down is the shape this loop takes when the layer is wrong, and
each round there costs a full boot.

Record every round in `rounds` — what you believed, what you changed, what the run said. That
list is the value delivered if the budget runs out.

On exhausting the budget: `git checkout .` in the worktree to return it to clean, and write
`{"result": "budget-exhausted", "rounds": [...]}`.

## Out shape

```json
{
  "result": "green",
  "rounds": [
    { "hypothesis": "...", "change": "...", "outcome": "still failed: <what qemu.log said>" }
  ],
  "changed_files": ["kernel/src/..."],
  "layer": "the layer actually patched, and why it owns the invariant",
  "generalizes": "why this is correct beyond the test — the other callers it also fixes, or that there are none",
  "fix_summary": "what the fix does, and which Asterinas abstraction it is designed against"
}
```

`layer` should normally match `diagnose.json`'s `owning_layer`. When it does not, say why the diagnosis
was wrong — that disagreement is worth reading, and silently patching elsewhere hides it.

`generalizes` is the anti-overfitting check written down. "Nobody else reaches this path" is a valid
answer; "the test now passes" is not an answer to this field at all.

Done when: the selector run prints `All conformance tests passed.` for the target test, `patch.diff`
holds that fix, and `generalizes` states why it is not a special case — or `result` is
`budget-exhausted` with the worktree clean.
