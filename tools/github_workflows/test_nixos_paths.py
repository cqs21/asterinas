#!/usr/bin/env python3

# SPDX-License-Identifier: MPL-2.0

"""Regress NixOS path coverage by executing the workflow's selection shell.

Requires Python 3, PyYAML, Bash, Git and jq. Run from any working directory.
The selector stays in the workflow until the separate selection refactor.
"""

import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

import yaml


ROOT = Path(__file__).resolve().parents[2]


def workflow(name):
    # BaseLoader preserves the YAML 1.2 key `on` and scalar spellings.
    return yaml.load(
        (ROOT / ".github/workflows" / name).read_text(), Loader=yaml.BaseLoader
    )


def triggers(document, event, paths):
    patterns = document["on"][event].get("paths")
    if patterns is None:
        return True
    # These workflows use exact files, directory/** and single-component '*'.
    for pattern in patterns:
        if pattern.endswith("/**"):
            if any(path.startswith(pattern[:-2]) for path in paths):
                return True
        else:
            expression = re.escape(pattern).replace(r"\*", "[^/]*")
            if any(re.fullmatch(expression, path) for path in paths):
                return True
    return False


class NixosPathsTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.full = workflow("test_nixos_full.yml")
        cls.iso = workflow("test_iso_image.yml")
        cls.selector = next(
            step
            for step in cls.full["jobs"]["select-test-suites"]["steps"]
            if step.get("id") == "select"
        )
        cls.full_matrix = json.loads(cls.selector["env"]["FULL_TEST_MATRIX"])
        cls.all_suites = {suite["name"] for suite in cls.full_matrix}

    def select(self, paths, event="pull_request", suite="all"):
        with tempfile.TemporaryDirectory(prefix="nixos-paths-") as temporary:
            changed = Path(temporary) / "changed"
            output = Path(temporary) / "output"
            changed.write_text("".join(path + "\n" for path in paths))
            script = self.selector["run"].replace("/tmp/changed", str(changed))
            script = script.replace("${{ github.event_name }}", event)
            env = dict(os.environ, **self.selector["env"])
            env.update(MANUAL_TEST_NAME=suite, GITHUB_OUTPUT=str(output))
            result = subprocess.run(
                ["bash", "-e", "-o", "pipefail", "-c", script],
                cwd=ROOT,
                env=env,
                text=True,
                capture_output=True,
            )
            if result.returncode:
                return result, {}
            values = dict(line.split("=", 1) for line in output.read_text().splitlines())
            matrix = json.loads(values["matrix"])
            self.assertEqual(values["run"], "true" if matrix else "false")
            # Check resource/timeout metadata as well as suite names.
            self.assertTrue(all(item in self.full_matrix for item in matrix))
            return result, {item["name"] for item in matrix}

    def assert_selection(self, paths, expected, event="pull_request", suite="all"):
        result, actual = self.select(paths, event, suite)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(actual, expected)

    def assert_triggers(self, document, paths, expected=True):
        for event in ("pull_request", "push"):
            with self.subTest(event=event, paths=paths):
                self.assertEqual(triggers(document, event, paths), expected)

    def test_known_missing_nixos_makefile(self):
        """The remaining omission from CI optimization step 01."""
        paths = ["test/nixos/Makefile"]
        self.assert_triggers(self.full, paths)
        self.assert_triggers(self.iso, paths)
        self.assert_selection(paths, self.all_suites)

    def test_prebuilt_cache_lockfile_regression_4162(self):
        self.assert_triggers(workflow("push_cachix_prebuilt.yml"), ["flake.lock"])

    def test_dev_cache_workflow_name_regression_4162(self):
        self.assert_triggers(
            workflow("push_cachix_dev.yml"), [".github/workflows/push_cachix_dev.yml"]
        )

    def test_shared_inputs_trigger_and_select_all_even_with_local_suite(self):
        inputs = [
            ".cargo/config.toml",
            ".github/actions/test/action.yml",
            "Cargo.toml",
            "Cargo.lock",
            "Components.toml",
            "Makefile",
            "OSDK.toml",
            "VERSION",
            "rust-toolchain.toml",
            "flake.nix",
            "flake.lock",
            "distro/nixpkgs.nix",
            "kernel/src/lib.rs",
            "ostd/src/lib.rs",
            "osdk/src/main.rs",
            "osdk/Cargo.lock",
            "test/initramfs/Makefile",
            "test/initramfs/nix/default.nix",
            "test/nixos/Makefile",
            "test/nixos/Cargo.toml",
            "test/nixos/Cargo.lock",
            "test/nixos/common/framework/src/session.rs",
            "test/nixos/common/merge_nixos_config.sh",
            "tools/nixos/build_nixos.sh",
            "tools/nixos/run.sh",
            "tools/qemu_args.sh",
            "tools/run_virtiofsd.sh",
            "tools/net/qemu-ifup.sh",
        ]
        for path in inputs:
            with self.subTest(path=path):
                self.assert_triggers(self.full, [path])
                self.assert_triggers(self.iso, [path])
                self.assert_selection([path], self.all_suites)
                self.assert_selection(
                    [path, "test/nixos/tests/networking/src/main.rs"], self.all_suites
                )

    def test_outer_filters_and_selector_cover_same_inputs(self):
        expected = {"test/nixos/tests/"}
        for key in ("FULL_TEST_PATH_PREFIXES", "INFRA_PATH_PREFIXES"):
            expected.update(self.selector["env"][key].split())
        for event in ("pull_request", "push"):
            patterns = self.full["on"][event]["paths"]
            actual = {p[:-2] if p.endswith("/**") else p for p in patterns}
            # distro/nixpkgs.nix is already included by distro/.
            self.assertEqual(actual, expected - {"distro/nixpkgs.nix"})

    def test_iso_hello_and_driver_inputs(self):
        for path in (
            "test/nixos/tests/hello/src/main.rs",
            "test/nixos/tests/hello/Cargo.toml",
            "test/nixos/tests/hello/extra_config.nix",
            "test/nixos/common/test_macro/src/lib.rs",
            "distro/etc_nixos/configuration.nix",
            "distro/iso_image/default.nix",
            "test/nixos/tests/networking/Cargo.toml",
        ):
            self.assert_triggers(self.iso, [path])
        self.assert_triggers(
            self.iso, ["test/nixos/tests/networking/src/main.rs"], False
        )

    def test_local_and_multiple_suites(self):
        self.assert_selection(
            ["test/nixos/tests/networking/src/main.rs"], {"networking"}
        )
        self.assert_selection(
            [
                "test/nixos/tests/networking/src/main.rs",
                "test/nixos/tests/development-tools/extra_config.nix",
            ],
            {"networking", "development-tools"},
        )

    def test_distro_smoke_and_desktop_drm(self):
        self.assert_selection(["distro/etc_nixos/configuration.nix"], {"system-core"})
        for directory in ("modules/xfce", "overlays/desktop"):
            path = f"distro/etc_nixos/{directory}/default.nix"
            self.assert_selection([path], {"system-core", "drm"})
            self.assert_selection(
                [path, "test/nixos/tests/networking/src/main.rs"], {"networking", "drm"}
            )

    def test_schedule_and_manual_selection(self):
        self.assert_selection([], self.all_suites, "schedule")
        self.assert_selection([], self.all_suites, "workflow_dispatch")
        self.assert_selection([], {"networking"}, "workflow_dispatch", "networking")
        result, _ = self.select([], "workflow_dispatch", "unknown-suite")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Unknown suite name", result.stderr)

    def test_unrelated_changes_and_exact_file_names(self):
        for path in ("README.md", "book/src/SUMMARY.md", "Makefile.notes", "flake.lock.old"):
            self.assert_triggers(self.full, [path], False)
            self.assert_triggers(self.iso, [path], False)
            self.assert_selection([path], set())
        self.assert_selection([], set())
        # hello is covered by minimal/ISO, outside the full workflow's 19 suites.
        self.assert_selection(["test/nixos/tests/hello/src/main.rs"], set())
        for name in ("test_nixos_minimal.yml", "test_ci_paths.yml"):
            document = workflow(name)
            self.assertNotIn("paths", document["on"]["pull_request"] or {})
            self.assertNotIn("paths-ignore", document["on"]["pull_request"] or {})
            self.assertNotIn("if", document["jobs"][next(iter(document["jobs"]))])

    def test_push_uses_same_selection(self):
        self.assert_selection(["Makefile"], self.all_suites, "push")
        self.assert_selection(
            ["test/nixos/tests/networking/src/main.rs"], {"networking"}, "push"
        )

    def test_suite_matrix_and_original_test_commands(self):
        suites = {p.parent.name for p in (ROOT / "test/nixos/tests").glob("*/Cargo.toml")}
        self.assertEqual(self.all_suites, suites - {"hello"})
        job = self.full["jobs"]["run-test-suites"]
        self.assertEqual(job["strategy"]["fail-fast"], "false")
        command = job["steps"][-1]["with"]["run"]
        self.assertRegex(command, r"set -e\nmake nixos .*\nmake run_nixos ")
        self.assertEqual(len(self.full_matrix), 19)
        self.assertIn(
            "make iso NIXOS_TEST_SUITE=hello",
            self.iso["jobs"]["iso-test"]["steps"][1]["run"],
        )

    def test_renames_deletions_and_pr_merge_base(self):
        step = next(
            step
            for step in self.full["jobs"]["select-test-suites"]["steps"]
            if step.get("id") == "diff"
        )
        with tempfile.TemporaryDirectory(prefix="nixos-git-diff-") as temporary:
            repository = Path(temporary)

            def git(*args):
                return subprocess.check_output(
                    ["git", *args], cwd=repository, text=True, stderr=subprocess.PIPE
                ).strip()

            git("init", "-b", "main")
            git("config", "user.name", "CI regression test")
            git("config", "user.email", "ci@example.com")
            original = repository / "tools/nixos/old.sh"
            original.parent.mkdir(parents=True)
            original.write_text("unchanged content\n")
            git("add", ".")
            git("commit", "-m", "Add shared input")
            base = git("rev-parse", "HEAD")
            git("switch", "-c", "feature")
            (repository / "notes").mkdir()
            git("mv", "tools/nixos/old.sh", "notes/moved.sh")
            git("commit", "-m", "Move shared input outside selected paths")
            renamed = git("rev-parse", "HEAD")

            def changed(event, before, after):
                output = repository / "changed"
                values = {
                    "github.event_name": event,
                    "github.event.pull_request.base.sha": before,
                    "github.event.pull_request.head.sha": after,
                    "github.event.before": before,
                    "github.sha": after,
                }
                script = re.sub(
                    r"\$\{\{\s*(.*?)\s*\}\}",
                    lambda match: values[match[1]],
                    step["run"],
                ).replace("/tmp/changed", str(output))
                subprocess.run(
                    ["bash", "-e", "-o", "pipefail", "-c", script],
                    cwd=repository,
                    check=True,
                )
                return output.read_text().splitlines()

            for event in ("pull_request", "push"):
                paths = changed(event, base, renamed)
                self.assertEqual(set(paths), {"tools/nixos/old.sh", "notes/moved.sh"})
                self.assert_triggers(self.full, paths)
                self.assert_triggers(self.iso, paths)
                self.assert_selection(paths, self.all_suites)

            git("switch", "main")
            original.unlink()
            git("add", "-u")
            git("commit", "-m", "Delete shared input")
            deleted = git("rev-parse", "HEAD")
            for event in ("pull_request", "push"):
                paths = changed(event, base, deleted)
                self.assertEqual(paths, ["tools/nixos/old.sh"])
                self.assert_selection(paths, self.all_suites)
            # Main-only deletion must not pollute the feature's PR diff.
            self.assertEqual(
                set(changed("pull_request", deleted, renamed)),
                {"tools/nixos/old.sh", "notes/moved.sh"},
            )
            # The first push must include all files, even at the root commit.
            self.assertEqual(changed("push", "0" * 40, base), ["tools/nixos/old.sh"])

            local = repository / "test/nixos/tests/networking/extra_config.nix"
            local.parent.mkdir(parents=True)
            local.write_text("suite configuration\n")
            git("add", "test")
            git("commit", "-m", "Add local suite input")
            local_base = git("rev-parse", "HEAD")
            (repository / "test/nixos/tests/drm").mkdir()
            git(
                "mv",
                "test/nixos/tests/networking/extra_config.nix",
                "test/nixos/tests/drm/extra_config.nix",
            )
            git("commit", "-m", "Move input between existing suites")
            paths = changed("pull_request", local_base, git("rev-parse", "HEAD"))
            self.assert_selection(paths, {"networking", "drm"})


if __name__ == "__main__":
    unittest.main(verbosity=2)
