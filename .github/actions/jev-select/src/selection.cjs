// SPDX-License-Identifier: MPL-2.0

// Resolve only tasks whose semantic judgment is unavailable by paths.
const { buildPlan, isObject, sameArray, sameKeys, requireInput } = require('./tasks.cjs');

function prepareSelection(root, table, event, testedSha, outcome, outputs, report) {
  const ids = Object.keys(table.tasks).sort();
  const state = { run: Object.fromEntries(ids.map(id => [id, true])), status: 'fallback',
    tested_sha: testedSha, path_tasks: [], reason: 'selector-error' };
  if (table.policy === 'all') return { ...state, status: 'bypassed', reason: 'force-all' };
  if (event === 'push') return { ...state, status: 'planned', path_tasks: ids, reason: 'push-paths' };
  requireInput(event === 'pull_request', 'changes policy requires a PR or push event');
  // Failed actions can leave partial outputs; none of those decisions are usable.
  if (outcome === 'failure') return { ...state, path_tasks: ids };
  requireInput(outcome === 'success', 'PR selector did not execute');
  requireInput(isObject(outputs) && isObject(report), 'selector outputs and report are required');
  const plan = buildPlan(root, table, outputs.run, outputs.selected, outputs['has-tasks'], outputs.status,
    outputs['tested-sha'], testedSha, event);
  requireInput(report.version === 9 && report.tested_ref === 'merge', 'unsupported selector report contract');
  requireInput(report.tested_sha === testedSha && report.mode === table.mode && report.status === plan.status,
    'selector report differs from effective outputs');
  requireInput(sameKeys(report.tasks, ids), 'report tasks must match the scope');
  for (const id of ids) {
    const decision = report.tasks[id];
    requireInput(isObject(decision) && decision.run === plan.run[id], 'report decisions must agree with run');
    requireInput(Array.isArray(decision.reasons) && decision.reasons.length &&
      decision.reasons.every(reason => typeof reason === 'string'), 'report reasons are invalid');
  }
  requireInput(isObject(report.analysis), 'report analysis is required');
  const { fallback_scope: scope, fallback_tasks: pathTasks } = report.analysis;
  requireInput(Array.isArray(pathTasks) && pathTasks.every(id => typeof id === 'string'),
    'report fallback tasks must be an array of task IDs');
  requireInput(sameArray(pathTasks, [...new Set(pathTasks)].sort()) && pathTasks.every(id => ids.includes(id)),
    'report fallback tasks must be unique, sorted and in scope');
  if (plan.status === 'fallback') {
    requireInput(['global', 'partial'].includes(scope) && pathTasks.length, 'fallback report must name affected tasks');
    requireInput(scope !== 'global' || sameArray(pathTasks, ids), 'global fallback must include every task');
    requireInput(pathTasks.every(id => plan.run[id]), 'upstream fallback tasks must be retained');
  } else {
    requireInput(scope === 'none' && !pathTasks.length, 'non-fallback report must not name fallback tasks');
  }
  Object.assign(state, { run: plan.run, status: plan.status, path_tasks: pathTasks, reason: 'jev-unavailable' });
  if (plan.status === 'bypassed') {
    const common = report.tasks[ids[0]].reasons.filter(reason => ids.every(id => report.tasks[id].reasons.includes(reason)));
    requireInput(common.some(reason => ['protected-path', 'fork', 'missing-api-key', 'external-context-disabled'].includes(reason)),
      'unexpected PR bypass reason');
    // Upstream bypasses workflow edits before semantic analysis. Apply the configured paths locally.
    Object.assign(state, { status: 'fallback', path_tasks: ids,
      reason: common.includes('protected-path') ? 'upstream-protected-path' : 'jev-unavailable' });
  }
  return state;
}

module.exports = { prepareSelection };
