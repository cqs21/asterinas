---
name: select-conformance-test
description: Pick a not-yet-passing conformance test (blocklisted case) from a chosen suite, scoring difficulty from the blocklist so the easiest is preferred. Use standalone to choose what to work on next, or as the first stage of the fix-conformance workflow.
---

# select-conformance-test

Choose one **blocklisted** (not-yet-passing) conformance test to work on, from a
user-chosen **suite**, and write a `select.json`. Difficulty is judged **statically**
from the blocklist files — no suite is run here.

Read `../shared/conformance-workflow-contract.md` first (§1 layout, §2 `select.json`
schema, §0 vocabulary). This skill writes exactly one
file: `select.json` in the problem sub-directory.

## Inputs

- `suite` (**required**) — `gvisor` | `ltp` | `kselftest` | `xfstests`.
- `test` (optional) — a specific selector token. If given, skip auto-pick and
  describe *that* test. Sets `source = "user-specified"`.
- `exclude` (optional) — a list of tokens already attempted this run; never pick one
  of these in auto-pick mode.
- `out-dir` (optional) — the problem sub-directory to write `select.json` into.
  Defaults to the current directory.

## Where the blocklists live

All under `test/initramfs/src/conformance/<suite>/`:

| Suite | Blocklist location | Entry format | Selector token |
|---|---|---|---|
| gvisor | `gvisor/blocklists/<binary>` (one file per test binary) | one gtest case per line (`EpollTest.RegularFiles`) | the *binary* file name (`epoll_test`); narrow with a gtest filter |
| ltp | `ltp/testcases/blocked/*.txt` | one testcase id per line (`rename01`) | the testcase id |
| kselftest | `kselftest/blocklists` (single file) | `<dir>:<case>` or `<dir>:*` per line | the `<dir>:<case>` entry |
| xfstests | `xfstests/block.list` | one test id per line (`generic/084`) | the test id |

`#`-comments and blank lines are ignored. Comments frequently explain *why* a case is
blocked and link a PR/issue — this is the primary difficulty signal.

## Procedure

1. **Read the suite's blocklist(s).** Collect every blocklisted entry with its
   inline/preceding comment (the `#` lines immediately above it) and any URLs/issue
   ids in that comment.

2. **If `test` was given**, locate its blocklist entry (which file, which exact line).
   It may not be blocklisted at all — then `blocklist_file`/`blocklist_entry` are null
   (a currently-passing or brand-new test the user wants to (re)confirm). Set
   `source = "user-specified"` and still score difficulty from any comment. Skip to 4.

3. **Auto-pick** (`source = "auto-picked"`), excluding anything in `exclude`:
   - Score each candidate's **difficulty** from static signals only:
     - `easy` — comment points at a small, localized cause (a wrong errno, an
       off-by-one, a missing flag, a resolution/rounding issue) or has no comment but
       is a single narrow case.
     - `medium` — a bounded feature gap in one subsystem, or a comment describing a
       specific but non-trivial behavior mismatch.
     - `hard` — comments mentioning races, "not supported", large missing subsystems,
       architectural limitations, or many linked failed attempts.
   - Prefer `easy`. Break ties toward entries whose comment is concrete and cites a
     single syscall/behavior (more tractable) over bare entries with no context.
   - For **gvisor**, a candidate is a single gtest case; set `test` to its binary file
     name and `gvisor_filter` to that one case (so the fixer runs just it).
   - Pick exactly one.

4. **Write `select.json`** per the contract schema: `suite`, `test`, `gvisor_filter`
   (gvisor only, else omit/empty), `blocklist_file`, `blocklist_entry`, `source`,
   `difficulty`, `rationale` (cite the comment you used), `linked_refs`.

## Output

Standalone: report the chosen test, its difficulty, and the rationale in one short
paragraph, and note the `select.json` path. In the workflow, the orchestrator reads
`select.json` from disk.

Do not run any suite, build the kernel, or modify any file other than `select.json`.
