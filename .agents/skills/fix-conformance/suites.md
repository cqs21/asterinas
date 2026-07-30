# Conformance suites

Read only the section for the suite you are working on.

The runners live in `test/initramfs/src/conformance/<suite>/`. `make run_kernel` decides
pass/fail by grepping `qemu.log` for `All conformance tests passed.`, so a green exit code
means the whole run passed; read `qemu.log` for the per-case lines either way.

`CONFORMANCE_TEST_SELECTOR` is comma-separated and, when non-empty, runs exactly those tests
and **ignores the blocklist** — that is what lets you run a still-blocked test, and what makes
one boot able to probe several candidates. Empty selector means "whole suite minus blocklist":
the CI path, and what phase 4 verifies. There is no third mode. All knobs are documented in
the root `Makefile`'s auto-test options block.

## LTP

- **Selector token:** the testcase id, e.g. `truncate02`.
- **Host baseline:** run the prebuilt binary directly out of the Nix store. Several LTP
  versions may be present, so resolve the pinned one — read `version` from
  `test/initramfs/nix/conformance/ltp.nix`, then
  `ls -d /nix/store/*-ltp-<version>/testcases/bin`.
- **Test source:** the LTP source tree in the store —
  `for d in /nix/store/*-source; do [ -d "$d/testcases/kernel/syscalls" ] && echo $d; done`
- **Candidate pools, in priority order:**
  1. `testcases/all.txt` — entries commented out with `# `. ~1000 of 1545; never enabled.
     Enable by **uncommenting** the line.
  2. `testcases/blocked/ext2.txt` — enabled but failing under ext2. Enable by deleting the line.
  3. `testcases/blocked/exfat.txt` — same, under exfat.
- **Filtering happens at packaging time**, not runtime: a blocked binary is not even copied
  into the initramfs. The selector reaches through the Nix boundary to package it anyway.
- **Which blocklist applies is chosen by `CONFORMANCE_TEST_WORKDIR`**, not by
  `CONFORMANCE_TEST_EXTRA_BLOCKLISTS`: `/ext2` → `blocked/ext2.txt`, `/exfat` →
  `blocked/exfat.txt`, anything else (default `/tmp`) → no blocklist.
- **Verify configuration:** from `all.txt`, the default (no workdir override). From
  `blocked/ext2.txt`, `CONFORMANCE_TEST_WORKDIR=/ext2`. From `blocked/exfat.txt`,
  `CONFORMANCE_TEST_WORKDIR=/exfat`.

## gvisor

- **Selector token:** a test *binary* name, e.g. `epoll_test`. Narrow to gtest cases inside it
  with `CONFORMANCE_TEST_GVISOR_FILTER=EpollTest.CloseFile:EpollTest.Oneshot` (`:`-separated).
- **Granularity:** the unit of work is the **case**, not the binary. `run_gvisor_test.sh` builds
  its filter two ways — with a selector it runs `--gtest_filter=$CONFORMANCE_TEST_GVISOR_FILTER`
  (defaulting to `*`), and without one it turns the blocklist into a *negative* filter
  `-Case1:Case2:...`. So deleting 2 of a binary's 5 blocked lines enables exactly those 2 and
  leaves the other 3 blocked. One run may target any subset of a binary's cases.
- **Note:** `CONFORMANCE_TEST_GVISOR_FILTER` is global, not per-binary — it applies to every
  binary in the selector. Keep the selector to one binary when filtering cases.
- **Host baseline:** `/root/syscall_test_bins/<binary> --gtest_filter=<gvisor_filter>` — the same
  filter string the run uses (243 binaries; path is `GVISOR_PREBUILT_DIR` from the Docker image).
  `--gtest_list_tests` with that filter shows what it matches without running anything.
- **Test source:** not vendored locally — read it from the gvisor repo
  (`test/syscalls/linux/<name>.cc`) on the web.
- **Candidate pool:** `blocklists/<binary>` — one file per binary, one gtest case per line,
  `#` comments allowed. ~540 blocked cases. Enable by deleting the case's line.
- **Also:** `blocklists.ext2/` and `blocklists.exfat/` are *extra* blocklist directories,
  selected with `CONFORMANCE_TEST_EXTRA_BLOCKLISTS=blocklists.ext2`. `TESTS` in the suite's
  `Makefile` lists the ~81 binaries actually packaged, out of the 243 prebuilt — a binary
  absent from `TESTS` is a whole-binary candidate, enabled by adding it to that list.
- **Verify configuration:** default if the case was in `blocklists/`; otherwise the matching
  `CONFORMANCE_TEST_WORKDIR=/ext2 CONFORMANCE_TEST_EXTRA_BLOCKLISTS=blocklists.ext2` (or the
  exfat pair).

## kselftest

- **Selector token:** `<collection>:<case>`, e.g. `timers:posix_timers`.
- **Host baseline:** `ls -d /nix/store/*-kselftest-*[0-9]` (skip the `.drv`), then run
  `<store>/<collection>/<case>`. `<store>/kselftest-list.txt` is the full inventory of
  valid tokens.
- **Candidate pool:** `blocklists` — a single file, one `<dir>:<test>` per line; `<dir>:*`
  blocks a whole collection. ~91 entries. Enable by deleting the line.
- **Also:** `blocklists.ext2` / `blocklists.exfat` are extra blocklist *files*, appended via
  `CONFORMANCE_TEST_EXTRA_BLOCKLISTS=blocklists.ext2`.
- **Per-case timeout:** `KSELFTEST_TIMEOUT`, default 300s.
- **Verify configuration:** as gvisor — default, or the matching workdir + extra blocklist.

## xfstests

- **Selector token:** a test id, e.g. `generic/001`.
- **Candidate pool:** `block.list` (6 entries). Enable by deleting the line. `short.list` /
  `full.list` are the run lists, chosen with `XFSTESTS_RUNLIST`.
- **Needs block devices:** `XFSTESTS_TEST_DEV` (`/dev/vdd`), `XFSTESTS_SCRATCH_DEV`
  (`/dev/vde`), `XFSTESTS_DISK_SIZE` (12G).
- **Runner uses `set -u`:** every variable it reads must have a default.
- **Verify configuration:** default, with `XFSTESTS_RUNLIST=/opt/xfstests/short.list`.
