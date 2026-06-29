# LTP Conformance-Fix Report (dry-run)

Suite: `ltp` · Verify: `false` · Cases analyzed: 1 · Verified-pass: 0

## Summary

| Case | Syscall | Deviation Type | Risk | Confidence | Verify Verdict |
|------|---------|----------------|------|------------|----------------|
| [unlink08](./unlink08.md) | unlink | permission-missing | high | high | inconclusive (verify disabled) |

## Verified fixes (ready for Review Gate)

_None._ Verification was disabled for this run (`verify=false`), so no case reached verified-pass state.

## Needs attention

### unlink08 — inconclusive (verify disabled)

- **Root cause:** The unlink path never validates permissions on the parent directory before removing an entry. `Path::unlink` delegates straight to the dentry with no `check_permission(MAY_WRITE | MAY_EXEC)` on the directory inode, so the EACCES subtests (0555 unwritable parent, 0666 unsearchable parent) wrongly succeed. EISDIR enforcement in the fs layer is intact.
- **Fixable:** Yes — a one-spot DAC guard in `Path::unlink` mirroring the existing `new_fs_child` write check, extended to also require `MAY_EXEC`.
- **Blocker:** Verification disabled — fix is not built or run, so `targetCasePassed=false` and the verdict is inconclusive. Additionally, the `all.txt` regression guard is **not yet applied**: line 1743 still reads `# unlink08` and must be uncommented to `unlink08` to enable CI coverage.

See [unlink08.md](./unlink08.md) for full spec, evidence, diff, regression plan, and PR draft.
