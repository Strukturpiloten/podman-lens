#!/usr/bin/env python3
"""Replay frozen pre-adoption bootstrap shells and their trust/queue failure controls."""

from __future__ import annotations

import copy
import json
import os
import re
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# Historical repository fixture: the exact seven-job trusted base before bootstrap.
BASELINE_SHA = "861f8cd18eb2419b3074993e2de08518861ca0ff"
# The live caller now uses the shared schema-3 planner. Preserve the actual completed
# bootstrap transition as immutable historical evidence, rather than executing its
# old shell expectations against the new caller or weakening trust/queue assertions.
BOOTSTRAP_SHA = "3783b3faf85741b88f571e8bb4fa055a461a3cb8"


def bootstrap_source(path: str, revision: str = BOOTSTRAP_SHA) -> bytes:
    return subprocess.check_output(["git", "show", revision + ":" + path], cwd=ROOT)


WORKFLOW = bootstrap_source(".github/workflows/ci.yml").decode()
POLICY = json.loads(bootstrap_source("scripts/validation-policy.json"))


def workflow_step(name: str) -> str:
    match = re.search(
        r"(?ms)^      - name: " + re.escape(name) + r"\n(.*?)(?=^      - |^  [a-z]|\Z)",
        WORKFLOW,
    )
    assert match is not None, name
    return match[1]


def workflow_shell(name: str) -> str:
    match = re.search(r"(?ms)^        run: ([|>]-?)\n(.*)", workflow_step(name))
    assert match is not None, name
    body = textwrap.dedent(match[2])
    return " ".join(body.splitlines()) if match[1].startswith(">") else body


class BootstrapHarness:
    def __init__(self, directory: Path, repository: Path = ROOT) -> None:
        self.root = directory
        self.environment = {
            **os.environ,
            "BASE_SHA": BASELINE_SHA,
            "HEAD_SHA": BASELINE_SHA,
            "GITHUB_SHA": BASELINE_SHA,
            "GITHUB_WORKSPACE": str(repository),
            "GITHUB_OUTPUT": str(directory / "output"),
            "PYTHONDONTWRITEBYTECODE": "1",
        }

    def run(self, shell: str, **environment: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["bash", "-e", "-c", shell],
            cwd=self.root,
            env={**self.environment, **environment},
            capture_output=True,
            text=True,
            check=False,
        )

    def install_trusted(self, legacy: bool) -> Path:
        trusted = self.root / ".validation-base"
        for path in ("scripts/validation-plan.py", "scripts/validation-policy.json"):
            content = bootstrap_source(path, BASELINE_SHA if legacy else BOOTSTRAP_SHA)
            target = trusted / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(content)
        return trusted

    def plan(self, event: str) -> tuple[subprocess.CompletedProcess[str], dict[str, str]]:
        output = Path(self.environment["GITHUB_OUTPUT"])
        output.unlink(missing_ok=True)
        result = self.run(workflow_shell("Select exact-revision validation plan"), EVENT_NAME=event)
        fields = (
            dict(line.split("=", 1) for line in output.read_text().splitlines())
            if output.exists()
            else {}
        )
        return result, fields

    def gate(self, event: str, plan: dict, needs: dict) -> subprocess.CompletedProcess[str]:
        return self.run(
            workflow_shell("Check exact-plan aggregate"),
            EVENT_NAME=event,
            PLAN_JSON=json.dumps(plan),
            NEEDS_JSON=json.dumps(needs),
        )


def complete_needs() -> dict:
    return {job: {"result": "success"} for job in ("validation-plan", *POLICY["jobs"])}


class BootstrapWorkflowTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory(prefix="podman-bootstrap-contract-")
        self.addCleanup(temporary.cleanup)
        self.harness = BootstrapHarness(Path(temporary.name))

    def assert_complete_plan(self, event: str) -> dict:
        result, fields = self.harness.plan(event)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        plan = json.loads(fields["plan"])
        self.assertEqual(plan["jobs"], dict.fromkeys(POLICY["jobs"], True))
        self.assertEqual(plan["profile"], "full")
        self.assertEqual(fields["select_api_release_type"], "true")
        self.assertEqual(self.harness.gate(event, plan, complete_needs()).returncode, 0)
        return plan

    def assert_all_results_required(self, event: str, plan: dict) -> None:
        for job in complete_needs():
            for result in ("skipped", "failure", "cancelled", None):
                with self.subTest(event=event, job=job, result=result):
                    needs = complete_needs()
                    needs[job]["result"] = result
                    self.assertNotEqual(self.harness.gate(event, plan, needs).returncode, 0)
        for name in ("documentation", "api-release-type"):
            needs = complete_needs()
            del needs[name]
            self.assertNotEqual(self.harness.gate(event, plan, needs).returncode, 0)
        needs = complete_needs()
        needs["unplanned"] = {"result": "success"}
        self.assertNotEqual(self.harness.gate(event, plan, needs).returncode, 0)
        for mutation in ({"tested_sha": "b" * 40}, {"profile": "prose"}):
            self.assertNotEqual(
                self.harness.gate(event, {**plan, **mutation}, complete_needs()).returncode, 0
            )
        numeric_selection = copy.deepcopy(plan)
        numeric_selection["jobs"]["api-release-type"] = 1
        self.assertNotEqual(
            self.harness.gate(event, numeric_selection, complete_needs()).returncode, 0
        )

    def test_queue_complete_plan_and_required_results(self) -> None:
        self.assert_all_results_required("merge_group", self.assert_complete_plan("merge_group"))
        self.harness.environment["GITHUB_SHA"] = "not-an-exact-sha"
        self.assertNotEqual(self.harness.plan("merge_group")[0].returncode, 0)

    def test_actual_legacy_planner_and_gate_require_transition_fallback(self) -> None:
        trusted = self.harness.install_trusted(legacy=True)
        command = [
            sys.executable,
            str(trusted / "scripts/validation-plan.py"),
            "plan",
            "--policy-root",
            str(trusted),
            "--repository-root",
            str(ROOT),
            "--event",
            "pull_request",
            "--base-sha",
            BASELINE_SHA,
            "--head-sha",
            BASELINE_SHA,
            "--tested-sha",
            BASELINE_SHA,
            "--github-output",
            self.harness.environment["GITHUB_OUTPUT"],
        ]
        result = subprocess.run(command, capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        fields = Path(self.harness.environment["GITHUB_OUTPUT"]).read_text()
        self.assertNotIn("select_api_release_type=", fields)
        old_plan = json.loads(result.stdout)
        self.assertEqual(set(old_plan["jobs"]), set(POLICY["jobs"]) - {"api-release-type"})
        old_needs = {
            name: item for name, item in complete_needs().items() if name != "api-release-type"
        }
        gate = [
            sys.executable,
            str(trusted / "scripts/validation-plan.py"),
            "gate",
            "--policy-root",
            str(trusted),
            "--event",
            "pull_request",
            "--base-sha",
            BASELINE_SHA,
            "--head-sha",
            BASELINE_SHA,
            "--tested-sha",
            BASELINE_SHA,
            "--plan-json",
            json.dumps(old_plan),
        ]
        self.assertEqual(
            subprocess.run(
                [*gate, "--needs-json", json.dumps(old_needs)], capture_output=True, check=False
            ).returncode,
            0,
        )
        for outcome in ("success", "skipped"):
            incompatible = {**old_needs, "api-release-type": {"result": outcome}}
            self.assertNotEqual(
                subprocess.run(
                    [*gate, "--needs-json", json.dumps(incompatible)],
                    capture_output=True,
                    check=False,
                ).returncode,
                0,
            )
        self.assert_all_results_required("pull_request", self.assert_complete_plan("pull_request"))

    def test_missing_trusted_planner_remains_complete(self) -> None:
        self.assert_all_results_required("pull_request", self.assert_complete_plan("pull_request"))

    def test_current_trusted_base_uses_complete_candidate_inventory(self) -> None:
        self.harness.install_trusted(legacy=False)
        self.assert_all_results_required("pull_request", self.assert_complete_plan("pull_request"))

    def test_known_policy_without_classifier_retains_complete_fallback(self) -> None:
        for legacy in (True, False):
            with self.subTest(legacy=legacy):
                trusted = self.harness.install_trusted(legacy=legacy)
                (trusted / "scripts/validation-plan.py").unlink()
                plan = self.assert_complete_plan("pull_request")
                needs = complete_needs()
                needs["api-release-type"]["result"] = "skipped"
                self.assertNotEqual(self.harness.gate("pull_request", plan, needs).returncode, 0)

    def test_absent_policy_with_classifier_retains_complete_fallback(self) -> None:
        trusted = self.harness.install_trusted(legacy=False)
        (trusted / "scripts/validation-policy.json").unlink()
        self.assert_complete_plan("pull_request")

    def test_unknown_or_malformed_trusted_inventory_rejects_plan_and_gate(self) -> None:
        trusted = self.harness.install_trusted(legacy=False)
        plan = self.assert_complete_plan("pull_request")
        path = trusted / "scripts/validation-policy.json"
        unknown = copy.deepcopy(POLICY)
        unknown["jobs"].append("unreviewed")
        wrong_repository = {**POLICY, "repository": "another/repository"}
        for classifier_present in (True, False):
            if not classifier_present:
                (trusted / "scripts/validation-plan.py").unlink()
            for content in (
                "not-json",
                json.dumps(unknown),
                json.dumps(wrong_repository),
                json.dumps({**POLICY, "schema": 3}),
            ):
                with self.subTest(classifier_present=classifier_present, policy=content):
                    path.write_text(content)
                    self.assertNotEqual(self.harness.plan("pull_request")[0].returncode, 0)
                    self.assertNotEqual(
                        self.harness.gate("pull_request", plan, complete_needs()).returncode, 0
                    )

    def test_non_file_policy_cannot_be_treated_as_absent(self) -> None:
        trusted = self.harness.install_trusted(legacy=False)
        plan = self.assert_complete_plan("pull_request")
        (trusted / "scripts/validation-plan.py").unlink()
        path = trusted / "scripts/validation-policy.json"
        path.unlink()
        path.mkdir()
        self.assertNotEqual(self.harness.plan("pull_request")[0].returncode, 0)
        self.assertNotEqual(self.harness.gate("pull_request", plan, complete_needs()).returncode, 0)
        path.rmdir()
        path.symlink_to("absent-policy.json")
        self.assertNotEqual(self.harness.plan("pull_request")[0].returncode, 0)
        self.assertNotEqual(self.harness.gate("pull_request", plan, complete_needs()).returncode, 0)

    def test_api_classifier_propagates_child_failure_before_output(self) -> None:
        target = self.harness.root / "scripts/check-public-api.sh"
        target.parent.mkdir()
        shell = workflow_shell("Preserve pre-1.0 breaking-change history")
        for value in ("major", ""):
            target.write_text("#!/usr/bin/env bash\nprintf '%s' '" + value + "'\n")
            output = Path(self.harness.environment["GITHUB_OUTPUT"])
            output.unlink(missing_ok=True)
            result = self.harness.run(shell)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(output.read_text(), "release_type=" + value + "\n")
        target.write_text("#!/usr/bin/env bash\nexit 23\n")
        output.unlink()
        result = self.harness.run(shell)
        self.assertEqual(result.returncode, 23)
        self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
