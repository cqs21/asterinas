# PR workflow concurrency

Every workflow with a `pull_request` trigger uses workflow-level concurrency:

```yaml
concurrency:
  group: ${{ github.workflow }}-${{ github.event_name }}-${{ github.event.pull_request.number || github.run_id }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}
```

Runs of the same workflow for the same PR share a group across commits.
A newly queued run replaces a pending run and requests cancellation of a running run in that group.
Workflow-level concurrency covers preparation and all matrix jobs in the run.
Different PRs and workflows have separate groups.
Keep workflow names unique, including when compared without regard to case,
because GitHub treats concurrency group names as case insensitive.

The event name separates PR runs from other event types.
For non-PR events, the run ID also isolates each run,
so push, schedule, manual, and `workflow_run` runs do not replace each other while pending
or cancel each other while running.
Setting only `cancel-in-progress: false` with a shared group would still replace pending runs.

## Covered entry points

| Purpose | Workflow files |
| --- | --- |
| Architecture tests | `test_x86.yml`, `test_aarch64.yml`, `test_riscv.yml`, `test_loongarch.yml` |
| VM and distribution tests | `test_x86_firecracker.yml`, `test_iso_image.yml`, `test_nixos_minimal.yml`, `test_nixos_full.yml` |
| Tooling and configuration tests | `test_cargo_component.yml`, `test_devcontainer.yml`, `test_nix_flake.yml`, `check_licenses.yml`, `validate_scmls.yml` |
| Documentation and publication checks | `publish_api_docs.yml`, `publish_website.yml`, `publish_osdk_and_ostd.yml`, `publish_sctrace.yml` |
| Cache build checks | `push_cachix_dev.yml`, `push_cachix_prebuilt.yml` |

The publication and cache workflows use their existing PR checks or dry-run paths.
Their non-PR publication steps have independent groups for each run.
NixOS full retains its existing policy of cancelling PR runs only,
with the event name added to its group.

No PR workflow currently calls a reusable workflow.
The external reusable workflow calls in `publish_docker_images.yml` belong to a non-PR entry point.
If a PR entry point gains a reusable workflow,
give any concurrency inside the called workflow a distinct prefix:
the called workflow inherits the caller's `github.workflow` value,
so sharing the caller's cancellation group can cancel the caller itself.

The code review workflow keeps its separate job-level `aster-code-review-run-pr-*` group.
The benchmark workflows keep `max-parallel: 1` for their benchmark matrices.
That limit applies within each matrix, not across runs or other workflows.
TDX, VMX, xfstests full, Vsock, and publication workflows without PR triggers
retain their existing scheduling and resource behavior.

## Verify cancellation on GitHub

Use a validation PR containing the concurrency changes.
For a workflow with a `paths` filter,
make sure every update in the demonstration matches that workflow's filter.

1. Trigger a run and wait until a preparation step is in progress.
   Record its URL, run ID, attempt, event, head SHA, job/step name, and start time.
   Update the same PR to trigger the same workflow again.
   Confirm the first run becomes `cancelled` and the replacement continues.
2. Repeat while a build or test step is in progress.
   Also check that any pending work in the superseded run stops.
   If a separate run is waiting on the concurrency group,
   trigger another update and verify replacement of that pending run.
3. Run a second PR and another workflow for the first PR.
   Verify that their runs continue independently.
   Observe push, schedule, manual, and `workflow_run` runs where supported,
   including two non-PR runs of the same event type.
4. Record the replacement's result and tested checkout SHA,
   the superseded run's elapsed job time,
   and the time between the replacement being created and the old run completing.
   Keep preparation and test observations separate.

These are runtime acceptance checks.
YAML linting and expression evaluation alone do not demonstrate GitHub cancellation
or cleanup of a process on a runner.

## Historical diagnosis and current-version recovery

Native concurrency does not compare commit ancestry or PR head freshness.
An older run that arrives late, or a rerun of an older revision containing this policy,
can cancel a newer revision's run.
Concurrency ordering does not guarantee that the newest commit wins.
Reruns use the original event's SHA/ref and workflow version;
they do not pick up this policy if it was absent from that version.
If a new revision does not trigger the workflow because of its event or path filters,
the older run can continue.
A cancelled or historical run therefore cannot establish that the current PR version passed.

For historical diagnosis, prefer an existing `workflow_dispatch` entry point
on a dedicated branch or tag at the desired revision.
Its event and run ID isolate it from PR runs.
Check the workflow's manual steps first:
the `publish_*` and `push_cachix_*` manual paths can publish,
so they are not substitutes for a PR dry-run.
For a workflow with no suitable manual path,
use a separate diagnostic PR instead of rerunning old work on the active PR.

To recover current-version validation:

1. Read the PR's current head SHA and inspect the affected workflow's runs and attempts.
   For PR events, distinguish the PR head SHA from the synthetic merge commit checked out by default.
2. Cancel only obsolete runs belonging to that PR and workflow.
   If a run for the current PR version exists, rerun that run and verify its event and checkout SHA.
   Rerunning an old run does not retarget it to the current version.
3. If no run exists for the current version,
   trigger a fresh matching PR event, such as a new commit or a close/reopen where appropriate,
   after checking the workflow's event and path filters.
   A manual run can supplement diagnosis but has a different event context from the PR check.
4. Wait for the current-version run to finish and record its URL, attempt, and actual checkout SHA.

When this policy is first introduced, runs created from older workflow versions
keep their old groups or have no group.
NixOS full also changes its group format by adding the event name.
Cancel obsolete runs from those versions explicitly during rollout if needed.

## Rollback

Revert the concurrency change to restore the previous scheduling policy.
Restore NixOS full's original group when reverting its change.
Then inspect active runs and perform current-version recovery as described above:
reverting the configuration does not revive cancelled runs
or move existing runs into a different group.

GitHub's [concurrency documentation](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency)
and [rerun documentation](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/re-run-workflows-and-jobs)
describe the native scheduling and rerun behavior.
See also the [reusable workflow limitations](https://docs.github.com/en/actions/reference/workflows-and-actions/reusing-workflow-configurations#limitations-of-reusable-workflows).
