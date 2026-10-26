# Jev CI task definitions

This directory defines the verification tasks available to CI workflows.
A task describes what a check covers, which paths require it to run,
and which parameters its workflow needs to execute it.
The local [`jev-select`](../actions/jev-select/action.yml) action loads these definitions
and produces the execution plan consumed by the workflow.
Test commands and job dependencies remain in the workflows and their execution actions.

## Directory layout

```text
jev-ci/
├── README.md
└── tasks/
    ├── test_nixos_minimal.yml          # One task for this workflow
    └── test_nixos_full/                # Multiple tasks for this workflow
        ├── _defaults.yml              # Paths shared by every task in this directory
        ├── development-tools-python.yml
        ├── networking.yml
        └── ...
```

Task sources are named after their owning workflow file, without the `.yml` extension.
For `.github/workflows/test_example.yml`, choose one of these layouts:

| Layout | `task-source` input | Use |
| --- | --- | --- |
| One task file | `.github/jev-ci/tasks/test_example.yml` | Select an existing job or group of jobs together. |
| A directory of task files | `.github/jev-ci/tasks/test_example/` | Select checks independently, for example one task per test suite. |

Each workflow loads only its own task source.
Directory loading discovers the directly contained `.yml` files;
it does not recurse into subdirectories.
`_defaults.yml` is optional and contains only shared `force_paths`.
It is not a task and is not used by the single-file layout.

## How selection uses the definitions

The NixOS workflows use `mode: enforce`, so usable Jev skip decisions take effect.
Selection depends on the event and policy:

| Event and policy | Selection |
| --- | --- |
| PR with `changes` | Matching `force_paths` require execution; Jev judges the remaining tasks from their descriptions and the changes. |
| Push with `changes` | Match `force_paths` against the complete push's `before` → `after` changes, without calling Jev. |
| Schedule or manual run with `all` | Execute every task in the requested scope, without calling Jev. |

When Jev cannot provide a usable judgment, the affected tasks use path matching:
matched tasks run and unmatched tasks skip.
The pinned upstream also bypasses semantic analysis when a PR changes a workflow file;
the local action resolves that bypass using the configured paths.
Configuration errors, inconsistent selection results, and failures to complete a required Git comparison
fail the selection job and prevent dependent test jobs from running.

## Define tasks for another workflow

Create the task source matching the workflow's filename using one of the layouts above.
Every task supports these fields:

| Field | Purpose |
| --- | --- |
| `id` | Required task ID matching the task filename without `.yml`. Use `^[a-z][a-z0-9_-]{0,63}$`; reserved IDs such as `all` and `matrix` are rejected. |
| `description` | Required, nonempty description of the behavior verified, relevant dependencies, and coverage boundaries. Jev uses this to judge changes outside the forced paths. |
| `force_paths` | Optional array of repository-relative glob patterns. A matching change requires this task to run. Defaults to `[]`. |
| `execution` | Optional local parameter object copied into the selected task's matrix entry. Defaults to `{}` and is not sent to Jev. |

Top-level fields other than these four, duplicate YAML keys, and YAML aliases are rejected.
IDs must be unique within the task source.

For a workflow that executes its existing jobs unchanged, omit `execution`.
For a workflow that consumes a task matrix, define parameters such as:

```yaml
execution:
  target_arch: riscv64
  profile: release
  timeout_minutes: 30
```

The selected task produces a matrix entry such as
`{"task":"build","target_arch":"riscv64","profile":"release","timeout_minutes":30}`.
Other workflows can define their own parameters without modifying the loader.
Parameter names must match `^[A-Za-z_][A-Za-z0-9_-]{0,63}$` and cannot replace `task`.
Values support JSON-compatible strings, finite numbers, booleans, nulls, arrays, and objects;
`__proto__`, `prototype`, and `constructor` keys are rejected at every nesting level.
The consuming workflow must use the parameters and validate its executor's requirements.
A parameter named `timeout_minutes`, for example, sets no timeout until the workflow uses it.

Connect the task source to a selection job after checkout:

```yaml
- id: select
  uses: ./.github/actions/jev-select
  with:
    task-source: .github/jev-ci/tasks/test_example.yml
    execution-policy: ${{ (github.event_name == 'pull_request' || github.event_name == 'push') && 'changes' || 'all' }}
    mode: enforce
    api-key: ${{ secrets.JEV_API_KEY }}
    allow-external-context: 'true'
```

Keep the selection job's token at `contents: read` and pass the Jev key only to that job.
Forward the action outputs as job outputs and make execution jobs depend on selection:

| Execution pattern | Outputs to forward | Execution job condition and configuration |
| --- | --- | --- |
| Existing jobs for one task | `run` | Use `needs: select` and `if: ${{ fromJSON(needs.select.outputs.run).test_example }}` for task ID `test_example`. |
| One matrix entry per task | `has-tasks`, `matrix` | Use `needs: select`, `if: ${{ needs.select.outputs.has-tasks == 'true' }}`, and `strategy.matrix: ${{ fromJSON(needs.select.outputs.matrix) }}`. |

Use [`test_nixos_minimal.yml`](../workflows/test_nixos_minimal.yml)
and [`test_nixos_full.yml`](../workflows/test_nixos_full.yml) as complete integration examples.
Remove workflow-level `paths` filters if semantic selection should inspect changes outside the forced paths.
For a manual `task` input, forward it as `requested-task: ${{ inputs.task || 'all' }}`.
A specific task ID is accepted only with the `all` policy; the default scope is every task in the source.
Adding a task definition also requires corresponding execution jobs or a matrix consumer in the workflow.

## Write useful descriptions and force paths

Describe the commands' observable behavior and the dependencies that can affect it.
Distinguish actual test coverage from incidental build prerequisites.
For example, the minimal NixOS guest uses the stage-1 initrd from
`distro/etc_nixos/modules/core.nix`, rather than the user-space archive built under `test/initramfs`.
Its description explains that boundary while accounting for the current build prerequisite.

Keep `force_paths` focused on inputs strongly related to the task:
its tests, fixtures, configuration, execution action, and relevant application modules or overlays.
Avoid copying broad kernel or repository-wide build paths into every task.
PR changes outside these paths remain available for semantic judgment;
pushes and path fallback skip unmatched tasks.

Paths are explicit: the loader adds no automatic workflow, task-source, or action paths.
Declare the task's own file if editing its definition should force that task.
For multiple tasks, put shared paths in `_defaults.yml` and task-specific paths in each task file;
the effective paths are their union.
Include `_defaults.yml` in its own shared paths if editing it should force every task.
Only add `.github/workflows/**` when every workflow edit should force the affected tasks.

Use positive repository-relative globs, such as `test/nixos/tests/json-tools/**`.
Keep each `force_paths` list in alphabetical order.
Absolute paths, `..` traversal, and negated patterns are rejected.
Quote patterns starting with `*` so YAML treats them as strings.

## Check a task change

The selection job validates task configuration before calling Jev or creating an execution plan.
Task YAML changes do not require rebuilding the selector's JavaScript bundle.

When adding a task, check that its ID and execution parameters match the workflow,
its own inputs select it, shared inputs select the intended scope,
and unrelated paths do not force it.
Use a manual run with the new task ID to exercise its execution path,
and inspect the selection job summary and `jev-selection-select` artifact in PR or push runs.
Selection reports are retained for 14 days.
When changing the workflow integration, run `actionlint` on that workflow as well.
