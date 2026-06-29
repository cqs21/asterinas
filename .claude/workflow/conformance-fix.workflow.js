// SPDX-License-Identifier: MPL-2.0
//
// Conformance Fix Workflow (prototype)
// ====================================
//
// Goal: for conformance test cases that Asterinas does NOT yet pass (i.e. cases
// disabled in the suite blocklists), automatically analyze the root cause and
// propose a *verified* fix.
// architecture (overall-architecture.svg) onto a Dynamic Workflow:
//
//   Select        -> Task Orchestrator: pick blocklisted cases to work on
//   Spec          -> SyscallSpecAgent:   extract Linux semantics / errno / boundaries
//   Analyze       -> BugMiningAgent:      diff Linux vs Asterinas impl, locate defect
//   Patch         -> PatchGenerationAgent:propose a concrete diff
//   Regression    -> RegressionTestAgent: minimal repro + regression (un-blocklist)
//   Verify        -> Execution Sandbox:   apply + build + run; only PASS survives
//   Synthesize    -> PRDraftAgent:        PR draft + consolidated report (Review Gate)
//
// Each case flows through the pipeline independently, so a slow build for one
// case never blocks analysis of another.
//
// Inputs (Workflow `args`, all optional):
//   { suite:  "gvisor" | "ltp" | "kselftest" | "xfstests"   // default "gvisor"
//   , cases:  ["open_test/OpenTest.OTrunc", ...]            // explicit targets
//   , limit:  number                                         // auto-pick count (default 2)
//   , verify: boolean                                        // run build+test (default true)
//   , stamp:  string }                                       // run id for artifact names
//
// NOTE: Verify builds the kernel and runs the test in QEMU inside the project
// Docker dev container. That is the expensive phase — keep `limit` small.

export const meta = {
  name: 'conformance-fix',
  description: 'Analyze blocklisted conformance test cases and produce verified fix proposals',
  whenToUse: 'When you want to root-cause and fix conformance (gVisor/LTP/kselftest/xfstests) cases that Asterinas currently blocklists.',
  phases: [
    { title: 'Select',     detail: 'Pick blocklisted cases (from args or by scanning blocklists)' },
    { title: 'Spec',       detail: 'Extract Linux syscall semantics, errno, boundary & permission model' },
    { title: 'Analyze',    detail: 'Diff Linux vs Asterinas implementation, locate the defect' },
    { title: 'Patch',      detail: 'Generate a concrete fix diff' },
    { title: 'Regression', detail: 'Minimal repro + regression test (un-blocklist the case)' },
    { title: 'Verify',     detail: 'Apply + build + run in an isolated worktree; keep only passing fixes' },
    { title: 'Synthesize', detail: 'PR draft + consolidated report for the Review Gate' },
  ],
}

// ----------------------------------------------------------------------------
// Configuration
// ----------------------------------------------------------------------------

const SUITE = (args && args.suite) || 'gvisor'
const LIMIT = (args && args.limit) || 2
const EXPLICIT = (args && args.cases) || null
const DO_VERIFY = !(args && args.verify === false)
const STAMP = (args && args.stamp) || 'run'
const ARTIFACT_DIR = `.claude/workflow/artifacts/${SUITE}-${STAMP}`

const CONFORMANCE_DIR = 'test/initramfs/src/conformance'
const SUITE_DIR = `${CONFORMANCE_DIR}/${SUITE}`

// How each suite encodes an "unsupported" case, and how to un-block it. Shared by
// the Select and Verify phases so the workflow is correct across suites, not just
// gVisor. (Verified against the suite Makefiles/run scripts.)
const BLOCKLIST_FORMATS = `Per-suite "unsupported case" encoding:
- gvisor / kselftest: "<suite>/blocklists/<binary>" — one file per test binary; each non-comment
  line is a case (gVisor gtest filter such as "OpenTest.OTrunc", excluded via --gtest_filter=-).
  A case is UNSUPPORTED while its line is present; un-block = delete that line. Extra per-fs lists
  live in "<suite>/blocklists.ext2" and "blocklists.exfat".
- ltp: "ltp/testcases/all.txt" — one test name per line; a line commented with "#"
  (e.g. "# unlink08") is UNSUPPORTED — the Makefile strips "#" lines with \`awk '!/^#/ && NF'\`.
  Un-block = uncomment the line. Per-fs lists: "ltp/testcases/blocked/{ext2,exfat}.txt".
- xfstests: "xfstests/{short,full,block}.list" select cases; consult those files.`

// Suite-aware guidance for building and running ONLY the target case during Verify.
function verifyRunGuidance(binary) {
  if (SUITE === 'gvisor' || SUITE === 'kselftest') {
    return `Build+run ONLY "${binary}" by overriding TESTS in ${SUITE_DIR}/Makefile:
       make run_kernel AUTO_TEST=conformance CONFORMANCE_TEST_SUITE=${SUITE} TESTS=${binary}
   Then read the per-case gtest output to confirm the specific case passed.`
  }
  if (SUITE === 'ltp') {
    return `Run ONLY this LTP case to stay fast (running all of all.txt is far too slow). The LTP
   build emits every NON-commented name from ltp/testcases/all.txt into runtest/syscalls. Best
   approach: in the worktree, temporarily reduce all.txt to just the target test name "${binary}"
   (which also un-blocks it), then:
       make run_kernel AUTO_TEST=conformance CONFORMANCE_TEST_SUITE=ltp
   Confirm via the result log (see ltp/run_ltp_test.sh: it greps for "Total Failures: 0") that
   "${binary}" reports TPASS for every subtest.`
  }
  return `Inspect ${SUITE_DIR}/Makefile and the run_*.sh script to build+run ONLY "${binary}";
   do not run the whole suite. Confirm the specific case passed from its log output.`
}

// Prepended to every analysis-phase prompt: these agents inspect the repo but
// must NOT mutate it. Only the Verify phase (in an isolated worktree) applies
// changes, and only the report agent writes (to the artifact directory).
const READ_ONLY = `READ-ONLY: Do NOT edit, write, create, or delete any repository file, and do
NOT run git/build/format commands that change the tree. Inspect with read/search tools only and
return your result purely as structured data (any diff goes in the schema's text field).\n\n`

// ----------------------------------------------------------------------------
// Structured-output schemas
// ----------------------------------------------------------------------------

const SELECT_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['cases'],
  properties: {
    cases: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
        required: ['id', 'binary', 'gtestCase', 'syscall', 'blocklistFile'],
        properties: {
          id: { type: 'string', description: 'Stable slug, e.g. open_test__OpenTest_OTrunc' },
          binary: { type: 'string', description: 'Test binary / suite file, e.g. open_test' },
          gtestCase: { type: 'string', description: 'Case filter, e.g. OpenTest.OTrunc (empty if whole-binary)' },
          syscall: { type: 'string', description: 'Primary syscall under test, e.g. open' },
          blocklistFile: { type: 'string', description: 'Path to the blocklist file containing this entry' },
          reason: { type: 'string', description: 'Why this case was picked / any inline comment found' },
        },
      },
    },
  },
}

const SPEC_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['syscall', 'assertedBehavior', 'errorCodes'],
  properties: {
    syscall: { type: 'string' },
    summary: { type: 'string', description: 'One-paragraph Linux semantics relevant to this case' },
    assertedBehavior: { type: 'string', description: 'Exactly what the test case asserts' },
    params: { type: 'array', items: { type: 'object', additionalProperties: false,
      properties: { name: { type: 'string' }, semantics: { type: 'string' } } } },
    errorCodes: { type: 'array', items: { type: 'object', additionalProperties: false,
      properties: { errno: { type: 'string' }, when: { type: 'string' } } } },
    boundaryBehaviors: { type: 'array', items: { type: 'string' } },
    permissionModel: { type: 'string' },
    sources: { type: 'array', items: { type: 'string' }, description: 'man page / spec / source refs' },
  },
}

const FINDING_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['rootCause', 'riskLevel', 'deviationType', 'confidence', 'implFiles'],
  properties: {
    rootCause: { type: 'string', description: 'Why Asterinas fails this case' },
    riskLevel: { type: 'string', enum: ['low', 'medium', 'high', 'critical'] },
    deviationType: { type: 'string', enum: [
      'permission-missing', 'boundary', 'resource-leak', 'race',
      'errno-mismatch', 'unimplemented', 'semantics', 'test-infra', 'other',
    ] },
    confidence: { type: 'string', enum: ['low', 'medium', 'high'] },
    implFiles: { type: 'array', items: { type: 'string' }, description: 'Relevant Asterinas source files' },
    evidence: { type: 'array', items: { type: 'object', additionalProperties: false,
      properties: {
        file: { type: 'string' }, line: { type: 'string' },
        snippet: { type: 'string' }, explanation: { type: 'string' },
      } } },
  },
}

const PATCH_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['fixable', 'strategy'],
  properties: {
    fixable: { type: 'boolean', description: 'False if this is unimplemented/too large for a focused patch' },
    strategy: { type: 'string', description: 'How the fix works (or why not fixable)' },
    diff: { type: 'string', description: 'Unified diff against the repo, or empty if not fixable' },
    changedFiles: { type: 'array', items: { type: 'string' } },
    blocklistEntriesToRemove: { type: 'array', items: { type: 'string' },
      description: 'blocklist lines to delete so the case runs' },
    impact: { type: 'string' },
    risks: { type: 'string' },
  },
}

const REGRESSION_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['approach', 'removesBlocklistEntry'],
  properties: {
    approach: { type: 'string', description: 'How the fix is regression-guarded' },
    removesBlocklistEntry: { type: 'boolean', description: 'Whether un-blocklisting the case is the regression guard' },
    addsRegressionTest: { type: 'boolean' },
    testFile: { type: 'string', description: 'Path of any added regression test (e.g. under test/initramfs/src/regression)' },
    testDescription: { type: 'string' },
    reproSteps: { type: 'string' },
  },
}

const VERIFY_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['verdict', 'built', 'targetCasePassed'],
  properties: {
    verdict: { type: 'string', enum: ['pass', 'fail', 'inconclusive'] },
    built: { type: 'boolean' },
    testRan: { type: 'boolean' },
    targetCasePassed: { type: 'boolean', description: 'The previously-blocklisted case now passes' },
    noRegressions: { type: 'boolean', description: 'Other cases in the same binary still pass' },
    command: { type: 'string', description: 'Exact build+run command used' },
    notes: { type: 'string', description: 'Build/run output highlights, failures, surprises' },
  },
}

const PRDRAFT_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['title', 'body'],
  properties: {
    title: { type: 'string', description: 'Imperative, <=72 chars (AGENTS.md git rules)' },
    body: { type: 'string', description: 'Markdown PR description with motivation, fix, and test summary' },
  },
}

// ----------------------------------------------------------------------------
// Phase 0 — Select target cases (Task Orchestrator)
// ----------------------------------------------------------------------------

phase('Select')

const selectPrompt = EXPLICIT
  ? `${READ_ONLY}You are the Task Orchestrator for the Asterinas conformance-fix workflow.
The user explicitly requested these conformance cases for suite "${SUITE}":
${EXPLICIT.map((c) => `  - ${c}`).join('\n')}

${BLOCKLIST_FORMATS}

For each requested case, locate it in the suite's lists and confirm it is currently unsupported;
capture the exact "blocklistFile" path and any inline comment as "reason". Set "binary" to the
test binary / list entry (e.g. open_test, or the LTP test name like unlink08); "gtestCase" to the
case after a slash (e.g. OpenTest.OTrunc), or "" when the whole binary/test is the unit. Infer the
primary "syscall" from the name (e.g. open_test -> open, unlink08 -> unlink). Make "id" a slug with
no slashes/dots (e.g. open_test__OpenTest_OTrunc).`
  : `${READ_ONLY}You are the Task Orchestrator for the Asterinas conformance-fix workflow.
Pick ${LIMIT} currently-UNSUPPORTED conformance cases for suite "${SUITE}".

${BLOCKLIST_FORMATS}

Read the relevant list(s) under "${SUITE_DIR}" and pick from the currently-unsupported entries.
Prefer cases that (a) map to a single syscall implemented in kernel/src/syscall/, (b) look like a
focused semantic/permission/errno/boundary deviation rather than a huge unimplemented feature, and
(c) come from different binaries for diversity. Skip entries whose inline comment shows they are
test-infra issues (e.g. missing applet) unless nothing else is available.

Set "binary" to the test binary / list entry, "gtestCase" to the case after a slash (or "" for a
whole binary/test), "blocklistFile" to the file you found it in, infer "syscall", and make "id" a
slug with no slashes/dots (e.g. open_test__OpenTest_OTrunc).`

const selected = await agent(selectPrompt, { label: `select:${SUITE}`, schema: SELECT_SCHEMA })
const cases = (selected && selected.cases ? selected.cases : []).slice(0, EXPLICIT ? undefined : LIMIT)

if (cases.length === 0) {
  log(`No blocklisted cases selected for suite "${SUITE}". Nothing to do.`)
  return { suite: SUITE, cases: [], note: 'no cases selected' }
}
log(`Selected ${cases.length} case(s): ${cases.map((c) => c.id).join(', ')}`)

// ----------------------------------------------------------------------------
// Pipeline — Spec -> Analyze -> Patch -> Regression -> Verify
// Each item carries an accumulating record; stages short-circuit gracefully.
// ----------------------------------------------------------------------------

const results = await pipeline(
  cases,

  // ---- Spec (SyscallSpecAgent) ----
  async (c) => {
    const spec = await agent(
      `${READ_ONLY}You are the SyscallSpecAgent. Extract the Linux specification relevant to this
conformance case so a later agent can detect where Asterinas deviates.

Suite: ${SUITE}
Test binary: ${c.binary}
Case: ${c.gtestCase || '(whole binary)'}
Primary syscall: ${c.syscall}

Produce a precise, *Linux-authoritative* spec: parameter semantics, the full set of relevant
error codes and exactly when each is returned, boundary behaviors, and the permission model.
State precisely what THIS test case asserts — list each subtest/assertion and its expected
result. The most authoritative source for "asserted behavior" is the test's own source: read it
if available (LTP sources ship under the prebuilt nix store path; gVisor sources are upstream at
github.com/google/gvisor under test/syscalls/linux/). Otherwise ground claims in the man7.org man
pages. List your sources. Do NOT look at Asterinas internals yet.`,
      { label: `spec:${c.id}`, phase: 'Spec', schema: SPEC_SCHEMA },
    )
    return { case: c, spec, ok: !!spec }
  },

  // ---- Analyze (BugMiningAgent) ----
  async (rec, c) => {
    if (!rec || !rec.ok) return rec || { case: c, ok: false }
    const finding = await agent(
      `${READ_ONLY}You are the BugMiningAgent. Compare the Linux spec below against the Asterinas
implementation and locate the precise defect that makes this conformance case fail.

Case: ${c.binary} / ${c.gtestCase || '(whole binary)'} (syscall: ${c.syscall})

Linux spec (from SyscallSpecAgent):
${JSON.stringify(rec.spec, null, 2)}

Find the implementation in kernel/src/syscall/${c.syscall}.rs and the deeper logic it calls
(VFS in kernel/src/fs/, fs/utils, process/, net/, etc.). Identify the SPECIFIC deviation:
missing permission/capability check, wrong/absent errno, off-by-one or boundary handling,
resource not released, race, or genuinely unimplemented behavior. For each failing assertion in
the spec, trace the exact code path and show why it produces the wrong result — confirm the
deviation actually causes THIS case to fail rather than just being a nearby smell. Cite concrete
file:line evidence with short snippets. Classify deviationType, riskLevel, and your confidence.
Be skeptical: if the failure is a test-infra artifact or unimplemented subsystem, say so.`,
      { label: `analyze:${c.id}`, phase: 'Analyze', schema: FINDING_SCHEMA },
    )
    return { ...rec, finding, ok: !!finding }
  },

  // ---- Patch (PatchGenerationAgent) ----
  async (rec, c) => {
    if (!rec || !rec.ok) return rec || { case: c, ok: false }
    const patch = await agent(
      `${READ_ONLY}You are the PatchGenerationAgent. Propose a minimal, correct fix for this defect,
following Asterinas coding guidelines (AGENTS.md): safe Rust only in kernel/, checked
arithmetic, validate at boundaries, narrow visibility, no unwrap on fallible paths.

Case: ${c.binary} / ${c.gtestCase || '(whole binary)'} (syscall: ${c.syscall})
Finding (from BugMiningAgent):
${JSON.stringify(rec.finding, null, 2)}

Output a concrete unified diff into "diff" that \`git apply\` can consume: include "a/" and "b/"
path prefixes (repo-relative), correct @@ hunk headers, and >=3 lines of surrounding context so it
applies to the current tree. List every changed path in "changedFiles". In
"blocklistEntriesToRemove" put the exact line(s) to delete/uncomment in ${rec.case.blocklistFile}
so the case runs. If the defect is a large unimplemented feature out of scope for a focused patch,
set fixable=false and explain. Keep the change surgical; describe impact and risks. Do NOT include
the blocklist edit inside "diff" — it is applied separately by the verifier.`,
      { label: `patch:${c.id}`, phase: 'Patch', schema: PATCH_SCHEMA },
    )
    return { ...rec, patch, ok: !!patch }
  },

  // ---- Regression (RegressionTestAgent) ----
  async (rec, c) => {
    if (!rec || !rec.ok) return rec || { case: c, ok: false }
    if (rec.patch && rec.patch.fixable === false) return rec // nothing to guard
    const regression = await agent(
      `${READ_ONLY}You are the RegressionTestAgent. Define how this fix is regression-guarded.

Case: ${c.binary} / ${c.gtestCase || '(whole binary)'} (syscall: ${c.syscall})
Patch proposal:
${JSON.stringify(rec.patch, null, 2)}

For gVisor/kselftest the natural regression guard is removing the case from
${rec.case.blocklistFile} so CI runs it. If the upstream coverage is weak, also propose a
minimal C reproducer under test/initramfs/src/regression following the conventions there.
Provide concrete repro steps. Do not write files; just specify them precisely.`,
      { label: `regress:${c.id}`, phase: 'Regression', schema: REGRESSION_SCHEMA },
    )
    return { ...rec, regression }
  },

  // ---- Verify (Execution Sandbox + Review Gate) ----
  async (rec, c) => {
    if (!rec || !rec.ok) return rec || { case: c, ok: false }
    if (!DO_VERIFY) return { ...rec, verify: { verdict: 'inconclusive', built: false, targetCasePassed: false, notes: 'verify disabled' } }
    if (rec.patch && rec.patch.fixable === false) {
      return { ...rec, verify: { verdict: 'inconclusive', built: false, targetCasePassed: false, notes: 'not fixable, skipped' } }
    }
    const verify = await agent(
      `You are the Execution Sandbox verifier, running inside the Asterinas Docker dev container in
an ISOLATED git worktree (a fresh checkout — your edits and builds here cannot affect other work).
Apply the proposed fix and PROVE it makes the target case pass without regressing the others.

Case: ${c.binary} / ${c.gtestCase || '(whole binary)'} (syscall: ${c.syscall})
Patch diff to apply:
\`\`\`diff
${(rec.patch && rec.patch.diff) || '(no diff)'}
\`\`\`
Blocklist line(s) to remove/uncomment in ${rec.case.blocklistFile}:
${JSON.stringify((rec.patch && rec.patch.blocklistEntriesToRemove) || [])}

Steps:
  1. Apply the diff (try \`git apply\`; if it does not apply cleanly, make the equivalent edit by
     hand and record what you changed in "notes").
  2. Un-block the target case (per the format below) so it actually runs:
     ${BLOCKLIST_FORMATS.split('\n').map((l) => '     ' + l).join('\n').trim()}
  3. ${verifyRunGuidance(c.binary)}
  4. BUILD IS SLOW: a fresh worktree has no cached \`target/\`, so the kernel + initramfs rebuild
     plus QEMU boot will likely exceed a single 10-minute Bash call. Run the \`make run_kernel ...\`
     command as a BACKGROUND task (run_in_background) and poll its output until it finishes, rather
     than one long blocking call. Use the same environment a normal \`make run_kernel\` expects (the
     container provides prebuilt test dirs, KVM, etc.).
  5. Determine the verdict from the LOGGED RESULT of the specific case (e.g. the gtest "[  PASSED ]"
     / "[  FAILED ]" lines for ${c.gtestCase || c.binary}, or the LTP TPASS/TFAIL lines and the
     "Total Failures: 0" check), NOT merely the make exit code.

Set built, testRan, targetCasePassed, noRegressions, the exact command, and key log excerpts in
notes. verdict=pass ONLY if the target case passes and no previously-passing case in the same
binary regressed. Be honest: a build failure, timeout, or unclear log is fail/inconclusive, never
pass. Do NOT touch anything outside this worktree.`,
      { label: `verify:${c.id}`, phase: 'Verify', schema: VERIFY_SCHEMA, isolation: 'worktree' },
    )
    return { ...rec, verify }
  },
)

const records = results.filter(Boolean)

// ----------------------------------------------------------------------------
// Phase 6 — Synthesize: PR drafts for verified fixes + consolidated report
// ----------------------------------------------------------------------------

phase('Synthesize')

const verified = records.filter((r) => r.verify && r.verify.verdict === 'pass')

const drafts = await parallel(verified.map((r) => () =>
  agent(
    `You are the PRDraftAgent. Write a PR draft for this VERIFIED conformance fix, following
the Asterinas git conventions in AGENTS.md (imperative subject <=72 chars, "Add regression
tests for every bug fix").

Case: ${r.case.binary} / ${r.case.gtestCase || '(whole binary)'} (syscall: ${r.case.syscall})
Finding: ${JSON.stringify(r.finding)}
Patch: ${JSON.stringify(r.patch)}
Regression: ${JSON.stringify(r.regression || {})}
Verification: ${JSON.stringify(r.verify)}

The body must cover: motivation (the failing case + root cause), the fix, the regression guard,
and the verification command/result.`,
    { label: `prdraft:${r.case.id}`, phase: 'Synthesize', schema: PRDRAFT_SCHEMA },
  ).then((d) => ({ ...r, draft: d })),
))

const finalRecords = records.map((r) => {
  const withDraft = drafts.find((d) => d && d.case.id === r.case.id)
  return withDraft || r
})

// Write a consolidated, human-reviewable report (the Review Gate input).
await agent(
  `You are the report writer for the Asterinas conformance-fix workflow.

IMPORTANT — write artifacts INSIDE the repository, not your current working directory. First
resolve the repo root with \`git rev-parse --show-toplevel\` and treat the artifact directory as
"<repo-root>/${ARTIFACT_DIR}". Use that absolute path for every file you write.

Write a single consolidated Markdown report to "<repo-root>/${ARTIFACT_DIR}/REPORT.md" (create
directories as needed via the Write tool), and one detail file per case at
"<repo-root>/${ARTIFACT_DIR}/<case-id>.md".

Run context: suite=${SUITE}, verify=${DO_VERIFY}, cases analyzed=${finalRecords.length},
verified-pass=${verified.length}.

Here are the full per-case records (JSON):
${JSON.stringify(finalRecords, null, 2)}

REPORT.md must contain:
  1. A summary table: case | syscall | deviationType | riskLevel | confidence | verify verdict.
  2. A "Verified fixes (ready for Review Gate)" section listing each passing case with its PR
     draft title and a link to its detail file.
  3. A "Needs attention" section for cases that are not fixable / failed verify / inconclusive,
     each with the root cause and the blocker.
Each <case-id>.md must contain the Spec, Finding (with evidence), Patch (with the diff in a
\`\`\`diff block), Regression plan, Verification result, and PR draft (if any).

After writing, return a short plain-text summary (counts + the artifact directory path). Do not
echo the whole report.`,
  { label: 'report', phase: 'Synthesize' },
)

log(`Done: ${verified.length}/${finalRecords.length} case(s) verified. Artifacts in ${ARTIFACT_DIR}`)

return {
  suite: SUITE,
  artifactDir: ARTIFACT_DIR,
  analyzed: finalRecords.length,
  verified: verified.length,
  cases: finalRecords.map((r) => ({
    id: r.case.id,
    syscall: r.case.syscall,
    deviationType: r.finding && r.finding.deviationType,
    riskLevel: r.finding && r.finding.riskLevel,
    fixable: r.patch ? r.patch.fixable : null,
    verdict: r.verify && r.verify.verdict,
    prTitle: r.draft && r.draft.title,
  })),
}
