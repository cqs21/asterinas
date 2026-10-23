// SPDX-License-Identifier: MPL-2.0

// Deterministic selection for pushes and tasks without usable Jev decisions.
const { isUtf8 } = require('node:buffer');
const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');
const { minimatch } = require('minimatch');

const SHA = /^[a-f0-9]{40}$/;
const ZERO_SHA = '0'.repeat(40);
const GIT_TIMEOUT_MS = 30000;
const MAX_INVENTORY_BYTES = 32 * 1024 * 1024;
// Keep the version and options aligned with the pinned upstream policy.ts.
const MATCH_OPTIONS = { dot: true, nonegate: true, nocomment: true };

class InputError extends Error {}
class ComparisonError extends Error {
  constructor() {
    super('path comparison could not be completed');
  }
}

function requireInput(condition, message) {
  if (!condition) throw new InputError(message);
}

function selectPaths(root, table, eventName, event, testedSha, state) {
  requireInput(table.policy === 'changes', 'path selection requires the changes policy');
  requireInput(['push', 'pull_request'].includes(eventName), 'path selection requires a PR or push event');
  requireInput(SHA.test(testedSha) && testedSha !== ZERO_SHA, 'github.sha must be a commit SHA');
  const ids = Object.keys(table.tasks).sort();
  requireInput(Array.isArray(state.path_tasks) && state.path_tasks.length > 0 &&
    state.path_tasks.every((id, index, tasks) => ids.includes(id) && (index === 0 || tasks[index - 1] < id)),
    'path-selected tasks must be unique, sorted and in scope');
  requireInput(state.run && Object.keys(state.run).sort().join(',') === ids.join(',') &&
    Object.values(state.run).every(value => typeof value === 'boolean'), 'invalid initial task decisions');

  let before, head;
  let deleted = false;
  if (eventName === 'push') {
    requireInput(event && typeof event.deleted === 'boolean', 'invalid push payload');
    requireInput(typeof event.before === 'string' && SHA.test(event.before), 'invalid push before SHA');
    requireInput(typeof event.after === 'string' && SHA.test(event.after), 'invalid push after SHA');
    requireInput(event.deleted || event.after === testedSha, 'push after must equal github.sha');
    before = event.before;
    head = event.after;
    deleted = event.deleted;
  } else {
    before = event?.pull_request?.base?.sha;
    head = event?.pull_request?.head?.sha;
    requireInput(typeof before === 'string' && SHA.test(before) && before !== ZERO_SHA, 'invalid PR base SHA');
    requireInput(typeof head === 'string' && SHA.test(head) && head !== ZERO_SHA, 'invalid PR head SHA');
    requireInput(state.status === 'fallback', 'PR path selection requires fallback status');
  }

  let changes = [];
  if (!deleted) {
    requireInput(before !== ZERO_SHA, 'push before is zero; path comparison requires a base commit');
    if (eventName === 'pull_request') verifyMerge(root, before, head, testedSha);
    changes = changedFiles(root, before, testedSha);
  }

  const status = eventName === 'pull_request' ? 'fallback' : 'planned';
  const run = { ...state.run };
  const decisions = {};
  for (const id of ids) {
    if (!state.path_tasks.includes(id)) {
      decisions[id] = { run: run[id], reasons: ['upstream-decision'] };
      continue;
    }
    const task = table.tasks[id];
    const matched = changes.some(change => task.force_paths.some(pattern =>
      minimatch(change.path, pattern, MATCH_OPTIONS)));
    const taskReason = deleted ? 'branch-deleted' : matched ? 'path-match' : 'no-path-match';
    run[id] = !deleted && matched;
    decisions[id] = { run: run[id], reasons: [taskReason] };
  }
  const selected = ids.filter(id => run[id]);
  return {
    kind: 'path-selection', version: 1, event_name: eventName, mode: 'paths',
    upstream_mode: table.mode, selection_reason: state.reason, path_tasks: state.path_tasks,
    status, before_sha: before, after_sha: deleted ? ZERO_SHA : testedSha,
    head_sha: head, diff_base_sha: before, tested_sha: testedSha,
    comparison_available: !deleted, changed_path_count: deleted ? null : changes.length,
    local_jev_calls: 0, ...(eventName === 'push' ? { jev_calls: 0 } : {}), tasks: decisions, run, selected,
    'has-tasks': selected.length ? 'true' : 'false',
  };
}

function git(root, args) {
  const result = spawnSync('git', ['-C', root, ...args], {
    timeout: GIT_TIMEOUT_MS, maxBuffer: MAX_INVENTORY_BYTES,
    env: { ...process.env, GIT_TERMINAL_PROMPT: '0', GIT_NO_REPLACE_OBJECTS: '1' },
  });
  if (result.error || result.status !== 0) throw new ComparisonError();
  return result.stdout;
}

function ensureCommit(root, sha) {
  try {
    git(root, ['cat-file', '-e', `${sha}^{commit}`]);
  } catch (error) {
    if (!(error instanceof ComparisonError)) throw error;
    git(root, ['fetch', '--no-tags', '--depth=1', 'origin', sha]);
    git(root, ['cat-file', '-e', `${sha}^{commit}`]);
  }
}

function verifyMerge(root, base, head, testedSha) {
  ensureCommit(root, testedSha);
  // Read raw parent headers: history traversal hides parents in shallow clones.
  const headers = git(root, ['cat-file', 'commit', testedSha]).toString('utf8').split('\n\n', 1)[0];
  const parents = headers.split('\n').filter(line => line.startsWith('parent ')).map(line => line.slice(7));
  requireInput(parents.length === 2 && parents[0] === base && parents[1] === head,
    'tested merge parents must equal the event base and head SHAs');
}

function changedFiles(root, before, after) {
  for (const sha of [before, after]) ensureCommit(root, sha);
  // Separate endpoints support both multi-commit and non-ancestor force pushes.
  // No rename detection: deleted and added paths both participate in matching.
  const result = git(root, ['diff', '--name-status', '--no-renames', '--no-ext-diff',
    '--no-textconv', '-z', before, after, '--']);
  if (!isUtf8(result)) throw new ComparisonError();
  if (!result.length) return [];
  const fields = result.toString('utf8').split('\0');
  if (fields.pop() !== '' || fields.length % 2 !== 0) throw new ComparisonError();
  const entries = [];
  for (let index = 0; index < fields.length; index += 2) {
    if (!/^[ACDMTUXB]$/.test(fields[index]) || !fields[index + 1]) throw new ComparisonError();
    entries.push({ status: fields[index], path: fields[index + 1] });
  }
  return entries;
}

function publishReport(report, env) {
  const directory = fs.mkdtempSync(path.join(env.RUNNER_TEMP, 'jev-paths-'));
  const reportPath = path.join(directory, 'report.json');
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2) + '\n', { mode: 0o600 });
  const summary = [
    '### Effective selection by paths', '',
    report.event_name === 'push'
      ? 'The pinned upstream does not support push analysis. Effective force_paths determine execution.'
      : 'Tasks without usable Jev decisions use force_paths. Other effective decisions are preserved.', '',
    `Status: **${report.status}**. Selected ${report.selected.length}/${Object.keys(report.run).length} tasks.`, '',
    `Comparison: \`${report.diff_base_sha}\` → \`${report.tested_sha}\`.`, '',
    '| Task | Run | Reason |', '| --- | --- | --- |',
    ...Object.entries(report.tasks).map(([id, decision]) =>
      `| ${id} | ${decision.run} | ${decision.reasons.join(', ')} |`), '',
  ];
  fs.appendFileSync(env.GITHUB_STEP_SUMMARY, summary.join('\n') + '\n');
  console.log(`Path selection: ${report.status}, ${report.selected.length}/${Object.keys(report.run).length} tasks; no additional Jev calls.`);
  return reportPath;
}

module.exports = { InputError, ComparisonError, selectPaths, changedFiles, publishReport };
