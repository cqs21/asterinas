# Specification of PR Review Skill

## Overview

A skill called `pr-review` that enables maintainers to generate,
edit, and submit GitHub PR reviews entirely from the CLI. The skill has
four subcommands: `new` (generate), `submit` (post to GitHub),
`redo` (follow-up review after PR updates), and `delete` (clean up).

The core workflow supports iterative review-revise cycles.

## Motivation

- Claude Code outputs review comments as console text, which cannot be posted to
  GitHub without manual copy-paste.
- Maintainers need to edit AI-generated reviews before posting.
- Reviews should be submitted as a single batch (pending review), not as
  individual comments, so the maintainer can finalize on GitHub.
- PRs often go through multiple review-revise iterations. The `redo`
  subcommand supports this by reading the previous review, checking which
  comments were addressed, and focusing on remaining and new issues.

## Workflow

```
Step 1: /pr-review new <PR>         Step 2: Edit <N>.md   Step 3: /pr-review submit <PR>
       |                                                          |
       v                                                          v
 1. Fetch PR metadata                                     1. Parse <N>.md
 2. Create worktree with PR source                        2. Validate format
 3. Fetch PR diff                                         3. Check staleness (head_sha)
 4. 7a: Prepare sub-agent context                         4. Create/find pending review
 5. 7b: Parallel sub-agent review                         5. Add comments (GraphQL)
 6. 7c: Merge + group + root-cause cluster                6. Verify comments landed
 7. 7d: Write draft review                                   (re-query review threads;
 8. 7e: Independent verification sub-agent                    report any missing)
 9. 7f: Apply fixes + finalize symlink                            |
       |                                                          v
       v                                                  Pending review on GitHub
 pr_reviews/<N>/review-<ts>.md                            (user finalizes on GitHub)
 pr_reviews/<N>.md (symlink)

                        Step 4: PR author updates the PR

Step 5: /pr-review redo <PR>        Step 6: Edit <N>.md   Step 7: /pr-review submit <PR>
       |                                                          |
       v                                                          v
 1. Read previous review                                  (same as Step 3)
 2. Re-fetch PR source
 3. Fetch new diff
 4. 7a-7f as above, with each
    sub-agent also checking
    addressed / unresolved / new
       |
       v
 pr_reviews/<N>/review-<ts2>.md
 pr_reviews/<N>.md (updated symlink)

 If satisfied → approve on GitHub.
 If not → edit <N>.md and go to Step 7, or wait for another revision and redo.
```

## Directory Structure

```
pr_reviews/
  .gitignore              # Contains "*" to ignore everything
  <N>/                    # Git worktree + review files for PR #N
    review-<ts1>.md       # First round review
    review-<ts2>.md       # Second round review (from redo)
    ...                   # (source files from the worktree)
  <N>.md                  # Symlink → <N>/review-<tsM>.md (latest)
```

Each PR review task has an associated worktree directory,
`pr_reviews/<N>`, which contains the source code of the target PR.
Having the whole source tree cloned gives code review more context.

Review files are stored inside the worktree directory with timestamped
names (`review-YYYYMMDD-HHMMSS.md`). Since `pr_reviews/.gitignore`
contains `*`, the review files are ignored by git even though they
sit inside the worktree.

A convenience symlink `pr_reviews/<N>.md` always points to the latest
review file. The `submit` subcommand reads from this symlink.

**Note on worktree recreation:** When `redo` removes and recreates the
worktree to pick up new code, the review files inside the worktree
directory are deleted. This is acceptable because:
1. The previous review has already been read before the worktree is
   recreated.
2. Previously submitted reviews are permanently recorded on GitHub.
3. The review files are working artifacts, not archival records.

## The Review File Format

The file serves two audiences simultaneously:

1. **Human reviewer** -- readable, editable markdown
2. **Submit parser** -- structured enough to extract file paths, line numbers,
   and comment bodies programmatically

### Full Example

````markdown
---
pr: 2887
repo: asterinas/asterinas
title: "Fix Metadata fields and pseudofs DeviceID"
head_sha: 40327e72abc123
event: comment
---

# PR Description

This PR refactors the `Metadata` struct fields with more descriptive names and fixes the issue that all pseudo filesystems report a `container_dev_id` of zero.

---

# Summary

Renames make the struct easier to read, and the `container_dev_id` fix is correct. Two issues need attention before merge: (1) `nr_sectors_allocated` assignment treats filesystem blocks as 512-byte sectors, so `st_blocks` will under-report by ~8× on ext2; (2) `SysFs` is missing the `Drop` impl that every other pseudo FS in this PR now has.

---

## `kernel/src/fs/ext2/inode.rs` line 113

> ```diff
> -            blocks: inner.blocks_count() as _,
> +            nr_sectors_allocated: inner.blocks_count() as _,
> ```

[**`accurate-names`**](https://asterinas.github.io/book/to-contribute/coding-guidelines/general-guidelines/index.html#accurate-names) (significant): `nr_sectors_allocated` is documented as 512-byte sectors, but `blocks_count()` returns filesystem blocks (4K on ext2). `st_blocks` will under-report by `blk_size / 512` — 8× for 4K blocks.

Fix:

```rust
nr_sectors_allocated: (inner.blocks_count() as usize) * (BLOCK_SIZE / 512),
```

---

## `kernel/src/syscall/statx.rs` lines 164-165

> ```diff
> -            stx_mtime: StatxTimestamp::from(info.ctime),
> +            stx_mtime: StatxTimestamp::from(info.last_meta_change_at),
> ```

**Pre-existing bug exposed by rename** (blocker): `stx_mtime` is the modification time; it should use `last_modify_at`, not `last_meta_change_at` (that's `stx_ctime`). The rename just made the existing bug visible.

Fix:

```rust
stx_mtime: StatxTimestamp::from(info.last_modify_at),
```

---

## `kernel/src/fs/sysfs/fs.rs`

**Missing `Drop` impl** (significant): Every other pseudo FS in this PR (`ProcFs`, `DevPts`, `RamFs`, `OverlayFs`, `PseudoFs`) gained a `Drop` that calls `release_container_dev_id()`. `SysFs` allocates an ID via `DeviceIdAllocator` but has no `Drop`, so a remounted `sysfs` leaks its ID.

Fix — add the same `Drop` impl as `ProcFs`:

```rust
impl Drop for SysFs {
    fn drop(&mut self) {
        DeviceIdAllocator::release_container_dev_id(self.dev_id);
    }
}
```
````

### Format Specification

#### YAML Frontmatter

Delimited by `---`. Required fields:

| Field      | Type   | Description                                            |
|------------|--------|--------------------------------------------------------|
| `pr`       | int    | PR number                                              |
| `repo`     | string | `owner/repo` slug                                     |
| `title`    | string | PR title (informational, not submitted)                |
| `head_sha` | string | HEAD commit SHA of the PR at generation time           |
| `event`    | string | One of: `comment`, `approve`, `request_changes`        |

The `event` field defaults to `comment`. The user can change it before
submitting. This maps to the GitHub Review API's `event` parameter.

#### PR Description Section

The `# PR Description` heading contains the PR author's description, copied
verbatim from GitHub. It is **not** submitted (the description already
exists on the PR). It serves two purposes:

1. Giving Claude context when generating the review.
2. Letting the human reviewer see what the PR author intended without
   leaving the terminal.

Because it is a regular markdown section (not YAML), it safely contains
arbitrary markdown content (code blocks, horizontal rules, tables, etc.)
without parsing issues.

Everything between `# PR Description` and the next `---` separator is the
description body.

#### Summary Section

The `# Summary` heading contains the top-level review body. This is posted
as the main review comment on GitHub (the text that appears above all inline
comments). It should be constructive and concise.

Everything between `# Summary` and the first `---` separator is the summary
body.

#### Comment Sections

Each comment is a `##` heading followed by a body, separated from the next
comment by `---`.

**Heading formats** (the parsing contract):

| Heading Format                         | Meaning                        | GitHub API Fields                            |
|----------------------------------------|--------------------------------|----------------------------------------------|
| `` ## `path/to/file.rs` line 42 ``    | Single-line comment            | `path`, `line: 42`, `side: RIGHT`            |
| `` ## `path/to/file.rs` lines 40-42 ``| Multi-line comment             | `path`, `start_line: 40`, `line: 42`         |
| `` ## `path/to/file.rs` line 42 (old)``| Comment on deleted code       | `path`, `line: 42`, `side: LEFT`             |
| `` ## `path/to/file.rs` ``            | File-level comment (no line)   | `path`, `subject_type: file`                 |

Rules:
- Each heading must contain exactly **one** backtick-wrapped path. Headings
  with multiple paths (e.g., `` ## `Makefile` / `foo.sh` ``) will not be
  parsed and the comment will be silently skipped.
- The backtick-wrapped path is always relative to the repo root.
- `line` refers to line numbers in the **file** (not diff-relative positions).
  For `RIGHT` side, this is the line number in the new version of the file.
  For `LEFT` side, this is the line number in the old version.
- The `(old)` suffix indicates the comment targets the left side of the diff
  (deleted or modified lines). Without it, the default is `RIGHT` (new code).

**Body structure:**

```
> ```diff
> - old line
> + new line
> ```

Comment text here. This is what gets posted to GitHub.
```

- **Blockquoted diff** (`> \`\`\`diff ... \`\`\``): Optional. Provides context
  for the human reviewer. This is **not** submitted to GitHub -- it is stripped
  during parsing.
- **Comment body**: Everything after the blockquoted diff (or after the heading
  if there is no diff block). This **is** submitted as the GitHub comment body.
  Supports full GitHub-flavored markdown.

**Comment body conventions:**

Each comment body must start with a short label that categorizes the issue. There are two label formats depending on whether the issue corresponds to a coding guideline:

1. **Guideline violation** -- cite the guideline as a hyperlink:

   ```
   [**`short-name`**](https://url/to/guideline.html#short-name) (severity): Explanation.
   ```

   To build the URL, take the relative path from the project's Guideline Index table (e.g., `rust-guidelines/language-items/functions-and-methods.md#small-functions`) and prepend `https://asterinas.github.io/book/to-contribute/coding-guidelines/`. Replace `.md` with `.html` in the final URL. Multiple guidelines can be cited in the same comment, each on its own paragraph. Do not repeat the guideline text -- the hyperlink already provides access.

2. **Non-guideline issue** -- use a bold short description:

   ```
   **Short description** (severity): Explanation.
   ```

`severity` is one of `blocker`, `significant`, or `minor`.

**Writing style (MANDATORY):**

- **No semantic line breaks.** Write each paragraph as a single unwrapped line. Do NOT hard-wrap at 72/80/100 columns inside a paragraph. Markdown viewers soft-wrap; hard-wrapping makes diffs noisy and forces unrelated lines to re-flow on every edit.
- **Be concise.** State the problem once, directly. Cut throat-clearing ("I noticed that..."), hedging ("perhaps could maybe..."), and restatements of the diff the reader is already looking at. Target 1-3 short sentences for the problem statement; the fix is the rest.
- **Every comment ends with a concrete fix when one exists.** A fix is a diff, code snippet, a one-liner command, or a specific API/flag to use. Put it at the end of the body, typically in a fenced code block introduced by a `Fix:` label. Only omit a fix when the right answer genuinely requires design discussion first, and say so explicitly ("No fix suggested: this needs a design decision on X before we know what to change.").
- **One root cause per comment.** If several symptoms share a single underlying bug or design flaw, do NOT file one comment per symptom. Merge them into a single comment that names the root cause once and gives the single fix that resolves all of them. Cross-reference the symptom lines in prose rather than repeating the analysis.

#### Delimiters

Each comment section is separated by a `---` (horizontal rule) on its own line.
The parser splits on these boundaries.

### Parsing Algorithm (for Submit)

```
1. Extract YAML frontmatter (between first pair of `---` lines)
2. Extract summary: text between `# Summary` and first `---` after it
3. For each `---`-delimited section with a `##` heading:
   a. Parse heading: extract path (in backticks), line/lines, (old) suffix
   b. Skip any blockquoted content (lines starting with `> `)
   c. Remaining text = comment body
   d. Skip sections where body is empty after stripping
4. Build API payload and POST
```

## Subcommand: `/pr-review new`

### Input

```
/pr-review new <pr_number_or_url>
```

Accepts:
- A PR number: `2887`
- A full GitHub URL: `https://github.com/asterinas/asterinas/pull/2887`

### Steps

1. **Resolve PR identity.**
   Extract the PR number (and optionally repo) from the input. If only a
   number is given, infer the repo from the current git remote.

2. **Fetch PR metadata.**
   ```bash
   gh pr view <N> --json number,title,headRefOid,baseRefName,headRefName,url,body
   ```

3. **Resolve the repo root.**
   All paths are relative to the repo root. Resolve it once:
   ```bash
   REPO_ROOT="$(git rev-parse --show-toplevel)"
   ```

4. **Create a git worktree with the PR's source.**
   Determine which git remote points to the repo that hosts the PR
   (compare remote URLs against the `owner/repo` from step 1). Typically
   this is `upstream` for upstream PRs or `origin` for fork PRs.
   ```bash
   git fetch <remote> +pull/<N>/head:refs/pr/<N>
   git worktree add "$REPO_ROOT/pr_reviews/<N>" refs/pr/<N>
   ```
   This gives Claude the full source tree of the PR at `pr_reviews/<N>/`
   without disturbing the user's working tree. The worktree reuses the
   existing repo's object store, so it is fast and costs no extra disk
   for git objects (only the checked-out files).

5. **Ensure `pr_reviews/.gitignore` exists.**
   Create `$REPO_ROOT/pr_reviews/.gitignore` containing `*` so that the
   entire directory is ignored by the main repo's git. This file is
   created once and persists across PR reviews.

6. **Fetch the PR diff.**
   ```bash
   gh pr diff <N>
   ```

7. **Generate the review file** via the parallel-sub-agent pipeline. The substeps 7a-7f match `SKILL.md`:

   **7a. Prepare sub-agent context.** Read the diff, identify the changed-files list and PR description, and assemble the shared briefing (diff, description, worktree path, review-file format spec).

   **7b. Parallel review.** Spawn several sub-agents in parallel, one per aspect (correctness, concurrency, API design and documentation, efficiency, coding guidelines, testing). Each returns a list of comments in the review-file format. For large PRs, cap at 6 in parallel; for very large ones, run sequentially in two waves to avoid context blowup.

   **7c. Merge.** Apply these passes in order:

   - *Deduplicate*: if two sub-agents found the same issue, keep the more complete version.
   - *Root-cause merge*: when several comments describe different symptoms of one underlying bug or design flaw, collapse them into a single comment that names the root cause and gives the single fix that resolves all symptoms.
   - *Group by subject*: order comments so that those targeting the same file, module, subsystem, or topic sit next to one another.
   - *Order by severity within each group*: blocker → significant → minor.
   - *Write the summary*: synthesize cross-cutting themes into the `# Summary` section — not a bulleted rehash of every comment.

   **7d. Write the draft.** Write the merged review to `$REPO_ROOT/pr_reviews/<N>/review-<TIMESTAMP>.md`. Follow the writing-style rules above (no semantic line breaks; concise; every comment ends with a fix when possible). Do not create the symlink yet.

   **7e. Verify with an independent sub-agent.** Spawn a fresh Agent with a clean context (not a continuation of any 7b sub-agent). Brief it with the draft review path, the worktree path, and instructions to verify:
   - Concrete claims (line numbers, file modes, behavior assertions, named symbols and flags)
   - Verdicts (is the severity calibrated correctly?)
   - Fix snippets (does the suggested code compile/parse/do what the comment says? Do the referenced flags/fields exist?)

   The sub-agent returns a structured list of confirmed / with-errors / unverifiable claims. Apply the reported fixes. For unverifiable claims, either hedge the language or cite a source; do not leave unhedged assertions the sub-agent could not verify.

   **7f. Finalize.** Create (or update) the convenience symlink:

   ```bash
   ln -sf "<N>/review-<TIMESTAMP>.md" "$REPO_ROOT/pr_reviews/<N>.md"
   ```

   Rationale for the verification step: sub-agents routinely hallucinate specific facts — line numbers drift, function names invert, fixes reference non-existent flags. A single independent pass on the merged draft catches most of these before they reach the user or GitHub, and is far cheaper than having the user discover them after `submit`.

8. **Report to user.** Print the path to the generated file and a short summary of how many comments were generated. Do not report until Step 7e's fixes have been applied.

### Idempotency

Running `new` again for the same PR removes the old worktree and recreates
it:
```bash
git worktree remove "$REPO_ROOT/pr_reviews/<N>" --force
git worktree add "$REPO_ROOT/pr_reviews/<N>" refs/pr/<N>
```
Previous review files inside the worktree are lost, but previously submitted
reviews are recorded on GitHub. A new review file is generated with a fresh
timestamp and the symlink is updated.

### Cleanup

When a review is no longer needed, use `/pr-review delete <N>` to remove
all artifacts. See the `delete` subcommand below.

## Subcommand: `/pr-review submit`

### Input

```
/pr-review submit <pr_number>
```

### Steps

1. **Resolve the repo root.**
   ```bash
   REPO_ROOT="$(git rev-parse --show-toplevel)"
   ```

2. **Locate and read** `$REPO_ROOT/pr_reviews/<N>.md`.
   This is a symlink to the latest review file. If it does not exist,
   report an error.

3. **Parse** the file according to the format specification above.
   Validate:
   - Frontmatter has required fields (`pr`, `repo`, `head_sha`, `event`)
   - At least the summary section exists
   - Each comment section has a valid heading

4. **Check for staleness.**
   Compare `head_sha` from frontmatter against the PR's current HEAD:
   ```bash
   gh pr view <N> --json headRefOid -q .headRefOid
   ```
   - If they match, proceed.
   - If they differ, **refuse to submit**. Print a message explaining that
     the PR has been updated since the review was generated, and instruct
     the user to run `/pr-review redo <N>` to generate a follow-up review.

   Rationale: posting comments on outdated line numbers is worse than
   re-reviewing. Auto-remapping line numbers across commits is fragile
   (files may be renamed, deleted, or rewritten) and would give false
   confidence.

5. **Submit via GitHub API.**
   The `scripts/submit_review.sh` script handles submission. It uses a
   two-step approach:

   **Step 5a: Create the review with inline comments (REST API).**
   ```
   POST /repos/{owner}/{repo}/pulls/{pull_number}/reviews
   ```
   ```json
   {
     "commit_id": "sha",
     "body": "Review summary",
     "comments": [
       {
         "path": "file.rs",
         "line": 42,
         "side": "RIGHT",
         "body": "Comment text"
       }
     ]
   }
   ```
   Inline comments use the line-based format (`line`/`side`, with
   optional `start_line`/`start_side` for multi-line comments).
   The review is always created as **PENDING** first, regardless of
   the `event` field in frontmatter. The user must explicitly finalize
   the review on GitHub.

   **Step 5b: Add file-level comments (GraphQL API).**
   The REST create-review endpoint does not support `subject_type: "file"`
   in its comments array. File-level comments are added after the review
   is created, using the GraphQL `addPullRequestReviewThread` mutation:
   ```graphql
   mutation {
     addPullRequestReviewThread(input: {
       pullRequestReviewId: "<node_id>",
       body: "Comment text",
       path: "file.rs",
       subjectType: FILE
     }) { thread { id } }
   }
   ```

6. **Verify submitted comments.** After all mutations complete, re-fetch the review's threads via GraphQL:

   ```graphql
   query {
     node(id: "<review_node_id>") {
       ... on PullRequestReview {
         comments(first: 100) {
           nodes { path line originalLine startLine originalStartLine subjectType body }
         }
       }
     }
   }
   ```

   For each expected comment (parsed from the review file), find a matching actual comment by `path` + `line` (or `subjectType == FILE` for file-level). Report any expected-but-missing comments, with (path, line) identifying each.

   Rationale: GitHub's GraphQL API sometimes accepts an `addPullRequestReviewThread` mutation but silently does not attach the thread — typically because the line is outside the diff hunk range, or the path has a case mismatch, or the mutation was rejected at a later phase. Re-querying the review is the only way to distinguish "accepted by API, visible on PR" from "accepted by API, silently dropped". This runs automatically inside `scripts/submit_review.sh`.

7. **Report results.**
   Print:
   - Number of comments submitted
   - Any comments that were skipped (with reasons)
   - The verification summary (expected vs. actual count, any missing)
   - A link to the PR for the user to finalize the review

   If verification reports missing comments, the agent must not claim the submit succeeded cleanly; it should relay the list and offer to re-submit after editing, or to add them manually on GitHub.

### Error Handling

| Error                                | Behavior                                              |
|--------------------------------------|-------------------------------------------------------|
| Review file not found                | Print error, suggest running `new` first              |
| Frontmatter missing required fields  | Print error, list missing fields                      |
| Comment line not found in diff       | Warn, skip that comment, continue                     |
| `head_sha` doesn't match current PR  | Refuse to submit, instruct user to run `redo`         |
| GitHub API error                     | Print error body, do not retry                        |
| Post-submit verification: missing    | Report (path, line) pairs not found on the review; suggest re-submitting after edit |

## Subcommand: `/pr-review redo`

### Input

```
/pr-review redo <pr_number_or_url>
```

### Purpose

After a PR author pushes updates in response to review feedback, the
reviewer runs `redo` to generate a follow-up review. Unlike `new`, which
starts from scratch, `redo` reads the previous review and uses it as
context to:

1. Verify whether each previous comment has been addressed.
2. Identify new issues introduced by the latest changes.
3. Carry forward unresolved comments with updated context.

### Steps

1. **Resolve PR identity.**
   Same as `new` step 1.

2. **Fetch PR metadata.**
   Same as `new` step 2.

3. **Resolve the repo root.**
   Same as `new` step 3.

4. **Read the previous review.**
   Read `$REPO_ROOT/pr_reviews/<N>.md` (the symlink to the latest review).
   Parse its contents to extract:
   - The previous `head_sha` (to know what code was reviewed before)
   - All comment sections (file, line, body)

   If no previous review exists, fall back to `new` behavior.

5. **Update the git worktree.**
   Re-fetch the PR ref and recreate the worktree:
   ```bash
   git fetch <remote> +pull/<N>/head:refs/pr/<N>
   git worktree remove "$REPO_ROOT/pr_reviews/<N>" --force
   git worktree add "$REPO_ROOT/pr_reviews/<N>" refs/pr/<N>
   ```
   The previous review files inside the worktree directory are lost,
   but the previous review contents were already read in step 4.

6. **Fetch the new PR diff.**
   ```bash
   gh pr diff <N>
   ```

7. **Generate the follow-up review** via the same sub-agent pipeline as `new` step 7 (sub-steps 7a-7f), with the following additions:

   - **7b (parallel review):** each aspect sub-agent receives the previous review alongside the new diff and source tree. For every previous comment in its aspect, it checks whether the issue was addressed in the new code; if addressed, it omits the comment; if not or only partially addressed, it includes the comment with a short note on what remains. It also surfaces any newly introduced issues.
   - **7c (merge):** the summary states how many previous comments were addressed, how many remain unresolved, and how many new issues were found.
   - **7e (verify):** the verification sub-agent additionally checks the "addressed" verdicts — for each previous comment marked as addressed, it confirms the fix is actually present in the new tree.

   Write the review to `$REPO_ROOT/pr_reviews/<N>/review-<TIMESTAMP>.md` and update the symlink:
   ```bash
   ln -sf "<N>/review-<TIMESTAMP>.md" "$REPO_ROOT/pr_reviews/<N>.md"
   ```

8. **Report to user.**
   Print:
   - Path to the new review file
   - Previous comments addressed vs. still unresolved
   - New issues found
   - Total comment count

   If all previous comments are addressed and no new issues are found,
   suggest approving the PR on GitHub.

### Difference from `new`

| Aspect              | `new`                          | `redo`                             |
|---------------------|--------------------------------|------------------------------------|
| Prior context       | None                           | Reads previous review              |
| Focus               | Full review from scratch       | Delta: addressed + new issues      |
| When to use         | First review of a PR           | After PR author pushes updates     |
| Fallback            | N/A                            | Falls back to `new` if no prior review |

## Subcommand: `/pr-review delete`

### Input

```
/pr-review delete <pr_number> [--yes]
```

The optional `--yes` flag skips the confirmation prompt.

### Purpose

Remove all local artifacts for a given PR review. This is the cleanup
command that should be run when a review is complete (merged or closed)
and the local files are no longer needed.

### Steps

1. **Resolve the repo root.**
   ```bash
   REPO_ROOT="$(git rev-parse --show-toplevel)"
   ```

2. **Ask for confirmation.**
   Unless `--yes` was passed, list what will be deleted and ask the user
   to confirm. If the user declines, abort without deleting anything.

3. **Remove the git worktree.**
   If a worktree exists at `$REPO_ROOT/pr_reviews/<N>`, remove it:
   ```bash
   git worktree remove "$REPO_ROOT/pr_reviews/<N>" --force
   ```
   This also deletes all review files inside the worktree directory.

4. **Remove the symlink.**
   ```bash
   rm -f "$REPO_ROOT/pr_reviews/<N>.md"
   ```

5. **Clean up the git ref.**
   ```bash
   git update-ref -d "refs/pr/<N>"
   ```

6. **Report to user.**
   Print a confirmation that all artifacts for PR #N have been removed.

### Idempotency

Running `delete` when no artifacts exist is a no-op (after confirmation).
Each step silently skips if the target does not exist.

## Skill Registration

The skill follows the [Agent Skills](https://agentskills.io) open standard.
The standard defines a portable format (`SKILL.md` in a named directory)
that is supported by Claude Code, Codex, Cursor, Gemini CLI, and many
other tools.

### Cross-Tool Compatibility

The Agent Skills standard defines these frontmatter fields:

| Field           | Required | Standard |
|-----------------|----------|----------|
| `name`          | Yes      | Yes      |
| `description`   | Yes      | Yes      |
| `license`       | No       | Yes      |
| `compatibility` | No       | Yes      |
| `metadata`      | No       | Yes      |
| `allowed-tools` | No       | Yes (experimental) |

Tool-specific extensions (ignored by other tools per the standard):

| Field                      | Tool        | Purpose                              |
|----------------------------|-------------|--------------------------------------|
| `argument-hint`            | Claude Code | Autocomplete hint for arguments      |
| `disable-model-invocation` | Claude Code | Prevent auto-invocation              |
| `user-invocable`           | Claude Code | Hide from `/` menu                   |
| `context`                  | Claude Code | Run in forked subagent               |

Codex uses a separate `agents/openai.yaml` file for its extensions
(e.g., `policy.allow_implicit_invocation`).

The standard requires that **unknown frontmatter fields are ignored**,
so including Claude Code-specific fields is safe for Codex (and vice
versa).

**Argument passing** differs between tools:
- Claude Code: `$ARGUMENTS`, `$0`, `$1` substitution in skill content
- Codex: arguments are appended to the prompt by the runtime

The `SKILL.md` body should be written so that it works regardless of how
arguments are injected. The instructions should say "the user provides
a subcommand (`new`, `submit`, `redo`, or `delete`) and a PR number or URL" rather
than relying solely on `$0`/`$1` substitution. For Claude Code, we can
additionally use `$0`/`$1` for convenience.

### Directory Layout

The canonical location is `.agents/skills/pr-review/`. Both Claude Code
and Codex discover skills from `.agents/skills/` at the repo root.

Claude Code additionally scans `.claude/skills/`. To keep a single source
of truth, `.claude/skills` is a symlink:

```
.agents/
  skills/
    pr-review/
      SKILL.md              # Skill prompt (the source of truth)
      spec.md               # This specification (supporting file)
      scripts/
        submit_review.sh    # Review submission script

.claude/skills -> ../.agents/skills    # Symlink for Claude Code
```

The `name` field in frontmatter must match the directory name (`pr-review`),
as required by the standard.

### SKILL.md Frontmatter

```yaml
---
name: pr-review
description: >
  Generate and submit GitHub PR code reviews. Use when the user wants
  to review a pull request, generate review comments, or submit review
  feedback to GitHub.
# Standard fields
compatibility: Requires gh (GitHub CLI) and git
# Claude Code extensions (ignored by other tools)
argument-hint: <new|submit|redo|delete> <pr_number_or_url>
disable-model-invocation: true
allowed-tools: Bash(gh *), Bash(git *), Read, Write, Glob, Grep, Agent
---
```

Key settings:
- **`description`** (standard): Describes what the skill does and when to
  use it. Must be under 1024 characters.
- **`compatibility`** (standard): Documents that `gh` and `git` are required.
- **`disable-model-invocation`** (Claude Code): Only the user can trigger
  this skill via `/pr-review`. The agent should never auto-invoke a review.
- **`allowed-tools`** (Claude Code): Pre-approves the tools needed so the
  user is not prompted repeatedly during review generation or submission.
- **`argument-hint`** (Claude Code): Shows `<new|submit|redo|delete> <pr_number_or_url>`
  in the autocomplete menu.

### Argument Handling

The skill accepts two arguments: a subcommand and a PR identifier.

In the `SKILL.md` body, instructions are written tool-agnostically:

```
The user provides:
- A subcommand: `new`, `submit`, `redo`, or `delete`
- A PR number (e.g., 2887) or GitHub URL
```

For Claude Code, the skill content additionally uses `$0` and `$1`:

```
Subcommand: $0
PR: $1
```

Examples of user invocation:
```
/pr-review new 2887
/pr-review new https://github.com/asterinas/asterinas/pull/2887
/pr-review submit 2887
/pr-review redo 2887
/pr-review delete 2887
```

### Supporting Files

The `SKILL.md` references this specification (`spec.md`) as a supporting
file for the review file format specification, parsing rules, and API
interaction details. The agent loads it when needed via relative path
reference, as recommended by the standard.

## Appendix: GitHub API Reference

### Create a Pull Request Review (REST)

```
POST /repos/{owner}/{repo}/pulls/{pull_number}/reviews
```

Request body (inline comments use line-based format; file-level comments
are not supported by this endpoint):
```json
{
  "commit_id": "sha",
  "body": "Review summary",
  "comments": [
    {
      "path": "file.rs",
      "line": 42,
      "side": "RIGHT",
      "body": "Comment text"
    },
    {
      "path": "file.rs",
      "line": 50,
      "side": "RIGHT",
      "start_line": 45,
      "start_side": "RIGHT",
      "body": "Multi-line comment"
    }
  ]
}
```

Note: omitting `event` creates a **PENDING** review.

### Add a File-Level Comment (GraphQL)

The REST create-review endpoint does not support `subject_type: "file"` in
its `DraftPullRequestReviewComment` type. File-level comments must be added
to an existing review via GraphQL:

```graphql
mutation {
  addPullRequestReviewThread(input: {
    pullRequestReviewId: "<review_node_id>",
    body: "File-level comment",
    path: "file.rs",
    subjectType: FILE
  }) { thread { id } }
}
```

### Submit a Pending Review (REST)

```
POST /repos/{owner}/{repo}/pulls/{pull_number}/reviews/{review_id}/events
```
```json
{
  "event": "COMMENT"
}
```

Reference: https://docs.github.com/en/rest/pulls/reviews#create-a-review-for-a-pull-request
