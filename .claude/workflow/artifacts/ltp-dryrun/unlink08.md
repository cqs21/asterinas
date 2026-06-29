# unlink08

- **Binary:** `unlink08`
- **Syscall:** `unlink`
- **Blocklist file:** `/root/asterinas/test/initramfs/src/conformance/ltp/testcases/all.txt`
- **Blocklist reason:** Entry found at line 1743 of `all.txt`, currently commented out (blocked/disabled) as `# unlink08` with no additional inline comment. LTP test case; the whole binary is the unit (no gtest-style case filter). Primary syscall inferred from binary name is `unlink`.

## Spec

**Syscall:** `unlink(2)`

`unlink()` deletes a name from the filesystem; if it is the last link to a file and no process has it open, the file's data is freed (if open, deletion is deferred until the last fd is closed). It operates on the *name* in the containing directory, so the relevant permission checks are against the parent directory, not the target file: the caller needs write permission on the directory holding the name (to remove the entry) and search (execute) permission on every directory component traversed to reach it. Linux refuses to unlink a directory entirely and reports `EISDIR` for that case (the non-POSIX value used since Linux 2.1.132; POSIX prescribes `EPERM`). When the parent directory has the sticky bit (`S_ISVTX`, e.g. `/tmp`) the caller may only unlink names it owns, or that are in a directory it owns, unless it has `CAP_FOWNER`. unlink08 specifically exercises the two EACCES cases (no write on parent, no search on parent) and the EISDIR case (target is a directory) for both root and an unprivileged user.

### Asserted behavior

unlink08 (LTP, master) asserts that `unlink(2)` FAILS with a specific errno in four scenarios, using `TST_EXP_FAIL` (verifies return == -1 AND errno == expected). Runs with `needs_root=1` and `needs_tmpdir=1`.

Setup:
- `SAFE_MKDIR("unwrite_dir",0777)`; `SAFE_TOUCH("unwrite_dir/file",0777)`; `SAFE_CHMOD("unwrite_dir",0555)` — parent dir loses write bit.
- `SAFE_MKDIR("unsearch_dir",0777)`; `SAFE_TOUCH("unsearch_dir/file",0777)`; `SAFE_CHMOD("unsearch_dir",0666)` — parent dir loses execute/search bit.
- `SAFE_MKDIR("regdir",0777)`. `pw = getpwnam("nobody")`.

Subtests (`tcases[]`):
1. `unlink("unwrite_dir/file")` as user nobody (forks child, `setuid(nobody)`) → must fail **EACCES** (no write permission on containing directory, mode 0555).
2. `unlink("unsearch_dir/file")` as user nobody → must fail **EACCES** (no search/execute permission on the containing directory, mode 0666).
3. `unlink("regdir")` as root (uid 0, runs directly) → must fail **EISDIR** (target is a directory).
4. `unlink("regdir")` as user nobody → must fail **EISDIR** (target is a directory).

Key correctness points implicitly checked: root is NOT exempt from the EISDIR rule (subtest 3 runs as root and still must get EISDIR, not success and not EPERM); the EACCES checks must be enforced against the *parent directory's* permissions for an unprivileged uid, and the missing-search-bit case (0666) must still yield EACCES rather than succeeding.

### Permission model

Permission is evaluated on the containing directory, not on the target file. To unlink a name the process needs (a) write permission on the directory that holds the name, and (b) execute/search permission on that directory and on every ancestor directory traversed during path resolution. The file's own mode is irrelevant. The superuser (uid 0 / `CAP_DAC_OVERRIDE`, `CAP_DAC_READ_SEARCH`) bypasses the directory write/search DAC checks, but does NOT bypass the EISDIR refusal to unlink a directory via `unlink()` — root must use `rmdir()`/`unlinkat(AT_REMOVEDIR)`. Sticky-bit rule: if the parent has `S_ISVTX`, a file may be unlinked only by a process whose EUID matches the file's owner or the directory's owner, or that holds `CAP_FOWNER`; otherwise EPERM/EACCES. Immutable/append-only files cannot be unlinked even by root (EPERM). unlink08 drops privilege via `setuid(nobody)` in a forked child for the two EACCES subtests and for subtest 4; subtest 3 runs as full root.

### Parameters

- `path (const char *)` — Pathname of the name to delete. Resolved relative to CWD if not absolute. Must NOT be a directory for plain `unlink()`: if it resolves to a directory, Linux returns EISDIR. If the final component is a symlink, the symlink itself is removed. Permission depends on the containing directory's mode. In unlink08 the values are `unwrite_dir/file`, `unsearch_dir/file`, and `regdir` inside the test's tmpdir.

### Error codes

| errno | When |
|-------|------|
| EACCES | No write access to the directory containing the path (cannot remove the entry), OR a directory in the path lacks search (execute) permission. unlink08 subtest 1 = no-write (0555), subtest 2 = no-search (0666), both as nobody. |
| EISDIR | path refers to a directory. Linux returns this (non-POSIX, since Linux 2.1.132) instead of EPERM, for BOTH privileged and unprivileged callers. unlink08 subtests 3 (root) and 4 (nobody). |
| EPERM | POSIX value for unlinking a directory (Linux uses EISDIR); OR fs disallows unlinking; OR file is immutable/append-only. Not directly asserted by unlink08. |
| EPERM/EACCES | Sticky bit (`S_ISVTX`) set and EUID is neither file owner nor dir owner, and no `CAP_FOWNER`. Not exercised. |
| ENOTDIR | A component used as a directory is not a directory. Not asserted. |
| ENOENT | A component does not exist / dangling symlink / empty path. Not asserted (all created in setup). |
| ELOOP | Too many symbolic links during translation. |
| ENAMETOOLONG | path (or component) too long. |
| EROFS | path on a read-only filesystem. |
| EBUSY | path in use by the system/another process (mount point, NFS silly-rename). |
| EFAULT | path outside accessible address space. |
| EIO | I/O error. |
| ENOMEM | Insufficient kernel memory. |
| EDQUOT | Disk quota exhausted (shared unlink/rmdir error on some systems). Not exercised. |

### Boundary behaviors

- EISDIR for a directory target applies even to root: privilege does not let `unlink()` remove a directory (subtest 3 runs as full root and must still get EISDIR). Returning 0 or EPERM would break the test.
- EACCES is decided by the PARENT directory's permission bits for an unprivileged EUID, independent of the target file's own mode (files are 0777 but still cannot be unlinked because the parent is 0555 / 0666).
- Missing search (execute) bit on the parent (mode 0666, subtest 2) must yield EACCES during path resolution of the final directory component, distinct from but same-errno as the missing-write case.
- Error precedence must match Linux: for subtests 1 and 2 the operation/lookup is denied → EACCES; for `regdir` the EISDIR type check applies (user has full 0777 access, so it is purely the directory-type refusal).
- Setup runs as root (`needs_root=1`); EACCES subtests fork and `setuid(nobody)`; subtest 3 stays root.
- An open file unlinked elsewhere stays alive until last close (core semantics, not tested here).
- Sticky-bit (`S_ISVTX`) ownership rule and immutable/append-only EPERM are part of the spec but not exercised.

### Sources

- man7.org `unlink(2)` (DESCRIPTION and ERRORS; EISDIR non-POSIX since Linux 2.1.132; sticky-bit rule; immutable/append-only EPERM).
- man7.org `path_resolution(7)` (search/execute on traversed directories; write on containing directory to remove a name).
- LTP source `unlink08.c` (linux-test-project/ltp master, `testcases/kernel/syscalls/unlink/unlink08.c`).
- Locally inspected binary at `/nix/store/z38is4k386vqhxq0j89b7chghkh5nlfw-ltp-20250930/testcases/bin/unlink08` (and conformance/initramfs copies).

## Finding

- **Root cause:** unlink path lacks parent-directory `MAY_WRITE`/`MAY_EXEC` permission check, so EACCES subtests pass when they should fail.
- **Risk level:** high
- **Deviation type:** permission-missing
- **Confidence:** high

**Implementation files:**
- `kernel/src/syscall/unlink.rs`
- `kernel/src/fs/vfs/path/mod.rs`
- `kernel/src/fs/vfs/path/dentry.rs`
- `kernel/src/fs/vfs/path/resolver.rs`
- `kernel/src/fs/fs_impls/ramfs/fs.rs`
- `kernel/src/fs/fs_impls/ext2/inode/dir/mod.rs`
- `kernel/src/fs/vfs/fs_apis/inode.rs`

### Evidence

**`kernel/src/syscall/unlink.rs` (lines 29-43)**
```
let (dir_path, name) = { ... ctx...resolver().read().lookup(&fs_path)? ...}; dir_path.unlink(name)?;
```
Resolves only the PARENT directory path, then unlinks the name in it. No permission check on `dir_path` before unlink.

**`kernel/src/fs/vfs/path/mod.rs` (lines 542-544)**
```
pub fn unlink(&self, name: &str) -> Result<()> { self.dentry.as_dir_dentry_or_err()?.unlink(name) }
```
`Path::unlink` delegates straight to dentry with no `check_permission(MAY_WRITE)` on the directory inode.

**`kernel/src/fs/vfs/path/mod.rs` (lines 64-71)**
```
pub fn new_fs_child(...) { if self.inode().check_permission(Permission::MAY_WRITE).is_err() { return_errno!(Errno::EACCES); } ...}
```
The create counterpart DOES check parent `MAY_WRITE` and returns EACCES; unlink is missing this exact guard, proving the asymmetry.

**`kernel/src/fs/vfs/path/dentry.rs` (lines 577-604)**
```
pub(super) fn unlink(&self, name: &str) -> Result<()> { ... let child_inode = self.remove_child(name, |dir_inode, name| dir_inode.unlink(name))?; ...}
```
Dentry-level unlink performs only dot/dotdot rejection and removal; no DAC check on the directory.

**`kernel/src/fs/fs_impls/ramfs/fs.rs` (lines 1016-1024)**
```
fn unlink(&self, name: &str) -> Result<()> { ... if target.typ == InodeType::Dir { return_errno_with_message!(Errno::EISDIR, "unlink on dir"); } ...}
```
EISDIR is enforced (subtests 3/4 pass) but no write/exec permission check on `self` (the parent dir), so subtests 1/2 wrongly succeed.

**`kernel/src/fs/vfs/path/resolver.rs` (lines 540-541)**
```
if path.inode().check_permission(Permission::MAY_EXEC).is_err() { return_errno_with_message!(Errno::EACCES, "the path cannot be looked up"); }
```
`MAY_EXEC` is only checked when traversing INTO a directory during lookup. The unlink target's parent is the terminal resolved component, so neither its exec bit (subtest 2, 0666) nor write bit (subtest 1, 0555) is ever validated.

**`kernel/src/fs/vfs/fs_apis/inode.rs` (lines 550-599)**
```
fn check_permission(&self, mut perm: Permission) -> Result<()> { ... if has_dac_override_capability(...) { perm -= MAY_READ | MAY_WRITE; ...} ... owner/group/other checks ...}
```
`check_permission` itself is correct and would yield EACCES for nobody on 0555/0666; the bug is that unlink never calls it on the parent inode.

## Patch

- **Fixable:** Yes
- **Strategy:** The unlink path never validated permissions on the parent directory before removing an entry. In Linux, deleting a name requires both write (`MAY_WRITE`) and search/execute (`MAY_EXEC`) permission on the containing directory. The create counterpart `Path::new_fs_child` already checks `MAY_WRITE` (its `MAY_EXEC` was implicitly validated during lookup traversal into the dir), but for unlink the parent directory is the terminal resolved component, so neither its write bit (0555 subtest) nor its exec bit (0666 subtest) was ever checked. Fix: add a DAC check in `Path::unlink` mirroring the existing `new_fs_child` guard, but checking `MAY_WRITE | MAY_EXEC` together so both EACCES subtests are caught. `Inode::check_permission` already correctly evaluates write and exec independently and honors `CAP_DAC_OVERRIDE`, owner/group/other modes, and fsuid/fsgid, so no per-fs changes (ramfs/ext2) are needed — the EISDIR enforcement in the fs layer is preserved and runs after this check passes. Safe Rust only, no `unwrap`, narrow change at the VFS boundary.
- **Impact:** Unlink/unlinkat now returns EACCES when the calling credentials lack write or search permission on the parent directory, matching Linux semantics. This unblocks LTP unlink08 (EACCES subtests). Normal unlink by owners/privileged callers is unaffected because `check_permission` grants access for those cases (and `CAP_DAC_OVERRIDE` bypasses write, plus exec when any exec bit is set).
- **Risks:** Low. The added check is identical in style to the existing create-path guard and uses the already-trusted `check_permission`. Callers that previously succeeded on directories without the exec bit will now correctly fail with EACCES — the intended Linux behavior. `rmdir` has the same latent gap but is out of scope for unlink08 and left unchanged to keep the patch surgical. The check runs before the dot/dotdot and EISDIR checks; for an unprivileged caller on a non-writable dir this means EACCES now takes precedence, which matches Linux.

**Changed files:**
- `/root/asterinas/kernel/src/fs/vfs/path/mod.rs`
- `/root/asterinas/test/initramfs/src/conformance/ltp/testcases/all.txt`

**Blocklist entries to remove:** `# unlink08`

```diff
diff --git a/kernel/src/fs/vfs/path/mod.rs b/kernel/src/fs/vfs/path/mod.rs
--- a/kernel/src/fs/vfs/path/mod.rs
+++ b/kernel/src/fs/vfs/path/mod.rs
@@ -540,6 +540,18 @@ impl Path {
     /// Unlinks a name from the `Path`.
     pub fn unlink(&self, name: &str) -> Result<()> {
+        // Removing an entry from a directory requires both write and search
+        // (execute) permission on that directory. Unlike `new_fs_child`, the
+        // execute bit is also checked here because the parent directory is the
+        // terminal component of path resolution, so its `MAY_EXEC` bit was not
+        // validated during lookup traversal.
+        if self
+            .inode()
+            .check_permission(Permission::MAY_WRITE | Permission::MAY_EXEC)
+            .is_err()
+        {
+            return_errno!(Errno::EACCES);
+        }
         self.dentry.as_dir_dentry_or_err()?.unlink(name)
     }
 
diff --git a/test/initramfs/src/conformance/ltp/testcases/all.txt b/test/initramfs/src/conformance/ltp/testcases/all.txt
--- a/test/initramfs/src/conformance/ltp/testcases/all.txt
+++ b/test/initramfs/src/conformance/ltp/testcases/all.txt
@@ -1740,7 +1740,7 @@
 
 unlink05
 unlink07
-# unlink08
+unlink08
 # unlink09
 # unlink10
```

## Regression plan

- **Removes blocklist entry:** Yes
- **Adds regression test:** No (upstream LTP coverage is sufficient)
- **Test file:** _(none)_

**Approach:** The natural and sufficient regression guard is to un-blocklist the upstream LTP case unlink08: change line 1743 of `all.txt` from `# unlink08` to `unlink08`. The LTP build Makefile (`test/initramfs/src/conformance/ltp/Makefile`) filters `all.txt` with `awk '!/^#/ && NF'`, so a commented line is never emitted into runtest/syscalls and the case never runs; uncommenting it adds it to the CI syscalls run. `run_ltp_test.sh` fails the run unless the log reports "Total Failures: 0", so a future regression in the unlink parent-dir permission check would make CI red.

Upstream coverage is strong and directly targets the fixed behavior, so NO custom C reproducer is warranted. Confirmed by running the prebuilt unlink08 binary on the host: 4 deterministic subtests — "unwritable directory : EACCES (13)" (0555 parent, missing MAY_WRITE), "unsearchable directory : EACCES (13)" (0666 parent, missing MAY_EXEC/search), and two "directory : EISDIR (21)" subtests. The new `check_permission(MAY_WRITE | MAY_EXEC)` in `Path::unlink` catches both EACCES subtests, while the fs-layer EISDIR check still runs afterward and preserves the two EISDIR subtests.

**State note:** the kernel fix is present in `path/mod.rs`, but the `all.txt` guard is **NOT yet applied** — line 1743 is still `# unlink08`. That one-line uncomment is the required regression-guard edit.

**Additional (out of patch scope, optional follow-up):** the gVisor blocklist `test/initramfs/src/conformance/gvisor/blocklists/unlink_test` still blocks `UnlinkTest.ParentDegradedPermissions`, the gVisor equivalent of unlink08's permission check, which would also be unblocked by this fix. Removing that entry would be a complementary guard, left for a separate change.

**Reproduction steps:**
1. Build initramfs/LTP and run the LTP syscalls conformance suite (`test/initramfs/src/conformance/ltp/run_ltp_test.sh`, which invokes `runltp -f syscalls`).
2. Pre-fix failure: with the kernel change reverted and line 1743 uncommented to `unlink08`, rebuild and run — unlink08 reports "unwritable directory" and "unsearchable directory" as TFAIL (unlink wrongly succeeds), so the log lacks "Total Failures: 0" and the harness exits non-zero.
3. Confirm fix: with the `Path::unlink` check present and line 1743 uncommented, rebuild and run — all 4 subtests TPASS (2x EACCES, 2x EISDIR), log shows "Total Failures: 0", harness exits 0.
4. Host sanity check (already performed): running the prebuilt `/opt/ltp/testcases/bin/unlink08` on a conformant kernel prints TPASS for all 4 subtests; Summary passed 4 failed 0.

## Verification result

- **Verdict:** inconclusive
- **Built:** false
- **Target case passed:** false
- **Notes:** verify disabled

## PR draft

**Title:** `fs: enforce parent-directory write/search permission on unlink (LTP unlink08)`

**Body:**

unlink previously removed a directory entry without checking permissions on the containing directory. Linux requires both write (`MAY_WRITE`) and search/execute (`MAY_EXEC`) permission on the parent directory to unlink a name. The create counterpart `Path::new_fs_child` already checks `MAY_WRITE`, but for unlink the parent is the terminal resolved component so neither its write nor exec bit was ever validated.

This adds a `check_permission(MAY_WRITE | MAY_EXEC)` guard in `Path::unlink`, returning `EACCES` when the caller's credentials lack either. `Inode::check_permission` already honors `CAP_DAC_OVERRIDE` and owner/group/other modes, so no per-fs changes are needed and the existing EISDIR enforcement is preserved (it runs after this check). Unblocks LTP `unlink08` (two EACCES subtests + two preserved EISDIR subtests).

Changed:
- `kernel/src/fs/vfs/path/mod.rs` — add the DAC guard.
- `test/initramfs/src/conformance/ltp/testcases/all.txt` — un-blocklist `unlink08`.

Note: verification was not run in this workflow (verify disabled); the `all.txt` un-blocklist edit still needs to be applied before merge.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
