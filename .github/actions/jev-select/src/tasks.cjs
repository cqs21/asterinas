// SPDX-License-Identifier: MPL-2.0

// Validate workflow-local configuration before it reaches Jev or execution jobs.
const fs = require('node:fs');
const path = require('node:path');
const YAML = require('yaml');

const TASK_ID = /^[a-z][a-z0-9_-]{0,63}$/;
const SHA = /^[a-f0-9]{40}$/;
const RESERVED_IDS = new Set([
  'all', '__proto__', 'prototype', 'constructor', 'toString', 'toLocaleString',
  'valueOf', 'hasOwnProperty', 'isPrototypeOf', 'propertyIsEnumerable',
  '__defineGetter__', '__defineSetter__', '__lookupGetter__', '__lookupSetter__',
  'run', 'selected', 'matrix', 'has-tasks', 'status', 'tested-sha', 'report-path',
].map(name => name.toLowerCase()));
const EXECUTION_KEY = /^[A-Za-z_][A-Za-z0-9_-]{0,63}$/;
const UNSAFE_KEYS = new Set(['__proto__', 'prototype', 'constructor']);
const NIXOS_EXECUTION_KEYS = ['suite', 'disk_size', 'timeout_minutes', 'test_timeout_minutes'];

class ConfigError extends Error {}
const isObject = value => value !== null && typeof value === 'object' &&
  [Object.prototype, null].includes(Object.getPrototypeOf(value));
const sameArray = (left, right) => Array.isArray(left) && left.length === right.length &&
  left.every((value, index) => value === right[index]);
const sameKeys = (object, keys) => isObject(object) && sameArray(Object.keys(object).sort(), [...keys].sort());

function requireInput(condition, message) {
  if (!condition) throw new ConfigError(message);
}

function readYaml(text) {
  const document = YAML.parseDocument(text, { uniqueKeys: true });
  requireInput(!document.errors.length && !document.warnings.length, 'invalid task YAML');
  YAML.visit(document, {
    Alias() { throw new ConfigError('YAML aliases are forbidden'); },
    Pair(_, pair) {
      requireInput(YAML.isScalar(pair.key) && typeof pair.key.value === 'string', 'YAML keys must be strings');
    },
  });
  return document.toJS();
}

function readJson(text) {
  try {
    const value = JSON.parse(text);
    // The existing YAML parser also detects duplicate keys in JSON objects.
    // JSON.parse alone silently accepts them, including contradictory decisions.
    const document = YAML.parseDocument(text, { uniqueKeys: true });
    requireInput(!document.errors.length, 'duplicate or invalid JSON key');
    return value;
  } catch {
    throw new ConfigError('invalid JSON input');
  }
}

function repositoryPath(root, relative) {
  requireInput(typeof relative === 'string' && relative !== '', 'repository path must be nonempty');
  requireInput(!relative.startsWith('/') && !relative.includes('\\'), 'repository path must be relative');
  const parts = relative.split('/');
  requireInput(parts.every(part => !['', '.', '..'].includes(part)), 'invalid repository path');
  let result = root;
  for (const part of parts) {
    result = path.join(result, part);
    requireInput(!fs.lstatSync(result, { throwIfNoEntry: false })?.isSymbolicLink(),
      'symbolic links are forbidden in task inputs');
  }
  return result;
}

function forcePaths(value) {
  requireInput(Array.isArray(value), 'force_paths must be an array');
  for (const pattern of value) {
    requireInput(typeof pattern === 'string' && pattern.trim(), 'force_paths entries must be nonempty strings');
    requireInput(!/^[!\/]|^[A-Za-z]:/.test(pattern), 'force_paths must be positive repository-relative globs');
    requireInput(!/[\\\x00-\x1f]/.test(pattern), 'force_paths must not contain control characters or backslashes');
    requireInput(pattern.split('/').every(part => !['', '.', '..'].includes(part)),
      'force_paths must not traverse directories');
  }
  return value;
}

function validateExecution(execution, root, workflow, taskId) {
  requireInput(isObject(execution), 'execution must be an object');
  requireInput(Object.keys(execution).every(key => EXECUTION_KEY.test(key) && key !== 'task'),
    'execution keys must be matrix identifiers other than task');
  validateExecutionValue(execution);
  // Keep this executor's domain checks without restricting other workflows' parameters.
  if (workflow !== 'test_nixos_full') return;
  requireInput(sameKeys(execution, NIXOS_EXECUTION_KEYS), 'NixOS execution requires exactly the suite, disk and timeout fields');
  requireInput(typeof execution.suite === 'string' && TASK_ID.test(execution.suite), 'invalid NixOS suite name');
  requireInput(execution.suite === taskId, 'NixOS suite must match its task id');
  requireInput(fs.statSync(repositoryPath(root, `test/nixos/tests/${execution.suite}`),
    { throwIfNoEntry: false })?.isDirectory(), 'NixOS suite directory is missing');
  for (const key of NIXOS_EXECUTION_KEYS.filter(key => key !== 'suite')) {
    requireInput(Number.isSafeInteger(execution[key]) && execution[key] > 0,
      'NixOS disk and timeouts must be positive integers');
  }
}

function validateExecutionValue(value) {
  if (value === null || ['string', 'boolean'].includes(typeof value)) return;
  if (typeof value === 'number') {
    requireInput(Number.isFinite(value), 'execution numbers must be finite');
  } else if (Array.isArray(value)) {
    value.forEach(validateExecutionValue);
  } else {
    requireInput(isObject(value), 'execution values must be JSON-compatible');
    for (const [key, entry] of Object.entries(value)) {
      requireInput(!UNSAFE_KEYS.has(key), 'execution contains a reserved object key');
      validateExecutionValue(entry);
    }
  }
}

function loadTasks(root, source, workflowFile, policy, requested, mode, event) {
  requireInput(['changes', 'all'].includes(policy), 'execution-policy must be changes or all');
  requireInput(['shadow', 'enforce'].includes(mode), 'mode must be shadow or enforce');
  requireInput(policy !== 'changes' || ['pull_request', 'push'].includes(event), 'changes policy requires a PR or push event');
  requireInput(policy === 'all' || requested === 'all', 'requested-task requires the all policy');
  requireInput(/^\.github\/workflows\/[a-zA-Z0-9_-]+\.yml$/.test(workflowFile), 'invalid owning workflow path');
  const workflow = path.basename(workflowFile, '.yml');
  const expected = `.github/jev-ci/tasks/${workflow}`;
  requireInput([`${expected}.yml`, expected, `${expected}/`].includes(source), 'task-source must match the owning workflow');
  const taskPath = repositoryPath(root, source.replace(/\/$/, ''));
  repositoryPath(root, workflowFile);
  let sharedPaths = [];
  let files;
  const readConfig = file => readYaml(fs.readFileSync(file, 'utf8'));
  if (!source.endsWith('.yml')) {
    requireInput(fs.statSync(taskPath, { throwIfNoEntry: false })?.isDirectory(), 'task source directory is missing');
    files = fs.readdirSync(taskPath).filter(name => name.endsWith('.yml')).sort().map(name => path.join(taskPath, name));
    for (const file of files) {
      requireInput(fs.lstatSync(file).isFile(), 'task files must be regular files');
    }
    const defaults = path.join(taskPath, '_defaults.yml');
    if (files.includes(defaults)) {
      const config = readConfig(defaults);
      requireInput(isObject(config) && Object.keys(config).every(key => key === 'force_paths'), 'defaults may only contain force_paths');
      sharedPaths = forcePaths(Object.hasOwn(config, 'force_paths') ? config.force_paths : []);
      files = files.filter(file => file !== defaults);
    }
  } else {
    requireInput(fs.statSync(taskPath, { throwIfNoEntry: false })?.isFile(), 'task source is missing');
    files = [taskPath];
  }
  requireInput(files.length, 'task source must contain at least one task');
  const tasks = {};
  for (const file of files) {
    const config = readConfig(file);
    requireInput(isObject(config) && Object.hasOwn(config, 'id') && Object.hasOwn(config, 'description') &&
      Object.keys(config).every(key => ['id', 'description', 'force_paths', 'execution'].includes(key)),
      'task requires id and description and must not contain unknown fields');
    const id = config.id;
    requireInput(typeof id === 'string' && TASK_ID.test(id) && !RESERVED_IDS.has(id), 'invalid or reserved task id');
    requireInput(!Object.hasOwn(tasks, id), 'duplicate task id');
    requireInput(id === path.basename(file, '.yml'), 'task id must match its filename');
    requireInput(typeof config.description === 'string' && config.description.trim(), 'task description must be nonempty');
    const paths = forcePaths(Object.hasOwn(config, 'force_paths') ? config.force_paths : []);
    const execution = Object.hasOwn(config, 'execution') ? config.execution : {};
    validateExecution(execution, root, workflow, id);
    tasks[id] = { description: config.description, force_paths: [...new Set([...sharedPaths, ...paths])].sort(), execution };
  }
  requireInput(requested === 'all' || Object.hasOwn(tasks, requested), 'unknown requested task');
  const scoped = Object.keys(tasks).sort().filter(id => requested === 'all' || requested === id);
  return { workflow, policy, mode, tasks: Object.fromEntries(scoped.map(id => [id, tasks[id]])) };
}

function upstreamTasks(table) {
  return Object.fromEntries(Object.entries(table.tasks).map(([id, task]) => [id,
    { description: task.description, force_paths: task.force_paths }]));
}

function buildPlan(root, table, run, selected, hasTasks, status, testedSha, expectedSha, event, pathTasks = []) {
  const tasks = table.tasks;
  requireInput(Array.isArray(pathTasks) && pathTasks.every(id => typeof id === 'string'), 'path-selected tasks must be an array of task IDs');
  requireInput(sameArray(pathTasks, [...new Set(pathTasks)].sort()) && pathTasks.every(id => Object.hasOwn(tasks, id)),
    'path-selected tasks must be unique, sorted and in scope');
  requireInput(!pathTasks.length || (table.policy === 'changes' && ['pull_request', 'push'].includes(event)),
    'path selection requires a PR or push changes policy');
  requireInput(!pathTasks.length || event !== 'pull_request' || status === 'fallback', 'PR path selection requires fallback status');
  for (const [id, task] of Object.entries(tasks)) validateExecution(task.execution, root, table.workflow, id);
  requireInput(sameKeys(run, Object.keys(tasks)), 'run must contain exactly the scoped task IDs');
  requireInput(Object.values(run).every(value => typeof value === 'boolean'), 'run decisions must be booleans');
  const effective = Object.keys(run).filter(id => run[id]).sort();
  requireInput(sameArray(selected, effective), 'selected must agree with run in sorted order');
  requireInput(hasTasks === (effective.length ? 'true' : 'false'), 'has-tasks must agree with selected');
  requireInput(['planned', 'bypassed', 'fallback'].includes(status), 'invalid selection status');
  if (['pull_request', 'push'].includes(event)) {
    requireInput(SHA.test(expectedSha) && testedSha === expectedSha, 'tested-sha must equal github.sha');
  }
  if (table.policy === 'all' || status === 'bypassed') {
    requireInput(Object.values(run).every(Boolean), 'all and bypass must retain every scoped task');
  }
  if (table.mode === 'shadow' && event !== 'push') {
    requireInput(Object.keys(tasks).filter(id => !pathTasks.includes(id)).every(id => run[id]),
      'PR shadow must retain tasks with usable Jev decisions');
  }
  if (event === 'push' && table.policy === 'changes') {
    requireInput(status === 'planned', 'push path selection requires a completed comparison');
  }
  return { run: Object.fromEntries(Object.keys(run).sort().map(id => [id, run[id]])), selected: effective,
    'has-tasks': hasTasks, matrix: { include: effective.map(id => ({ task: id, ...tasks[id].execution })) }, status };
}

function writeOutputs(file, outputs) {
  const lines = Object.entries(outputs).map(([key, value]) => {
    const text = typeof value === 'string' ? value : JSON.stringify(value);
    requireInput(!/[\r\n]/.test(text), 'output must fit on one line');
    return `${key}=${text}\n`;
  });
  fs.appendFileSync(file, lines.join(''));
}

module.exports = { ConfigError, SHA, isObject, sameArray, sameKeys, requireInput, readYaml, readJson,
  loadTasks, upstreamTasks, buildPlan, writeOutputs };
