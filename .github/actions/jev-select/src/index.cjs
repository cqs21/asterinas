// SPDX-License-Identifier: MPL-2.0

const fs = require('node:fs');
const path = require('node:path');
const { ConfigError, SHA, requireInput, readJson, loadTasks, upstreamTasks, buildPlan, writeOutputs } = require('./tasks.cjs');
const { prepareSelection } = require('./selection.cjs');
const { InputError, ComparisonError, selectPaths, changedFiles, publishReport } = require('./paths.cjs');

const readFile = file => readJson(fs.readFileSync(file, 'utf8'));

function main(env) {
  switch (env.INPUT_PHASE) {
    case 'load': return load(env);
    case 'plan': return plan(env);
    case 'diagnostics': return diagnostics(env);
    default: throw new ConfigError('unknown selector phase');
  }
}

function load(env) {
  // GITHUB_WORKFLOW_REF is owner/repo/.github/workflows/name.yml@ref.
  const workflowRef = env.GITHUB_WORKFLOW_REF.split('@', 1)[0].split('/').slice(2).join('/');
  const table = loadTasks(env.GITHUB_WORKSPACE, env.TASK_SOURCE, workflowRef, env.EXECUTION_POLICY,
    env.REQUESTED_TASK, env.SELECT_MODE, env.GITHUB_EVENT_NAME);
  const directory = fs.mkdtempSync(path.join(env.RUNNER_TEMP, 'jev-select-'));
  const tablePath = path.join(directory, 'execution.json');
  fs.writeFileSync(tablePath, JSON.stringify(table));
  writeOutputs(env.GITHUB_OUTPUT, { tasks: upstreamTasks(table), 'execution-table': tablePath });
}

function plan(env) {
  const table = readFile(env.EXECUTION_TABLE);
  let outputs, report;
  let reportPath = '';
  if (table.policy === 'changes' && env.GITHUB_EVENT_NAME === 'pull_request' && env.SELECTOR_OUTCOME === 'success') {
    outputs = { run: readJson(env.SELECT_RUN), selected: readJson(env.SELECT_SELECTED),
      'has-tasks': env.SELECT_HAS_TASKS, status: env.SELECT_STATUS, 'tested-sha': env.SELECT_TESTED_SHA };
    reportPath = env.SELECT_REPORT_PATH;
    report = readFile(reportPath);
  }
  const state = prepareSelection(env.GITHUB_WORKSPACE, table, env.GITHUB_EVENT_NAME, env.GITHUB_SHA,
    env.SELECTOR_OUTCOME, outputs, report);
  if (state.path_tasks.length) {
    report = selectPaths(env.GITHUB_WORKSPACE, table, env.GITHUB_EVENT_NAME,
      readFile(env.GITHUB_EVENT_PATH), env.GITHUB_SHA, state);
  } else if (table.policy === 'all') {
    report = { kind: 'local-policy', version: 1, mode: 'all', status: 'bypassed',
      tested_sha: state.tested_sha, comparison_available: false, run: state.run,
      selected: Object.keys(state.run).filter(id => state.run[id]).sort(), reason: 'force-all', jev_calls: 0 };
  }
  const run = state.path_tasks.length ? report.run : state.run;
  const selected = Object.keys(run).filter(id => run[id]).sort();
  const result = buildPlan(env.GITHUB_WORKSPACE, table, run, selected, selected.length ? 'true' : 'false',
    state.status, state.tested_sha, env.GITHUB_SHA, env.GITHUB_EVENT_NAME, state.path_tasks);
  // Publish only after the complete comparison and final plan have succeeded.
  if (state.path_tasks.length) {
    reportPath = publishReport(report, env);
  } else if (table.policy === 'all') {
    reportPath = path.join(path.dirname(env.EXECUTION_TABLE), 'all-report.json');
    fs.writeFileSync(reportPath, JSON.stringify(report, null, 2) + '\n', { mode: 0o600 });
    fs.appendFileSync(env.GITHUB_STEP_SUMMARY, '### Explicit task scope\n\n' +
      'The all policy selects every task in the requested scope without calling Jev.\n\n' +
      `Selected: ${selected.map(id => '`' + id + '`').join(', ')}.\n`);
  }
  writeOutputs(env.GITHUB_OUTPUT, { ...result, 'report-path': reportPath });
}

function diagnostics(env, log = console.log) {
  try {
    const report = readFile(env.SELECT_REPORT_PATH);
    requireInput(report.tested_sha === env.GITHUB_SHA, 'report tested SHA differs from github.sha');
    if (['path-selection', 'local-policy'].includes(report.kind)) {
      requireInput(report.version === 1, 'unsupported local report version');
      requireInput(report.comparison_available === true, 'local selection did not establish a comparison');
    } else {
      requireInput(report.version === 9, 'unsupported selector report version');
      requireInput(report.manifest?.hash != null, 'selector did not establish a comparison');
    }
    requireInput(typeof report.diff_base_sha === 'string' && SHA.test(report.diff_base_sha), 'comparison SHA is invalid');
    const entries = changedFiles(env.GITHUB_WORKSPACE, report.diff_base_sha, report.tested_sha);
    log(`Changed files for verified comparison ${report.diff_base_sha} -> ${report.tested_sha}:`);
    for (const entry of entries) log('changed-file ' + JSON.stringify(entry));
    log(`Changed-file entries: ${entries.length}`);
  } catch {
    // This diagnostic does not alter the already verified execution plan.
    log('Changed-files diagnostic unavailable: no verified comparison or immutable Git objects could be read.');
  }
}

module.exports = { main, diagnostics };
if (require.main === module) {
  try {
    main(process.env);
  } catch (error) {
    // Raw Git, parser and provider errors can contain untrusted input or secrets.
    const message = error instanceof ConfigError || error instanceof InputError || error instanceof ComparisonError
      ? error.message : 'task selection could not be completed';
    console.error(`jev-select: ${message}`);
    process.exitCode = 1;
  }
}
