#!/usr/bin/env python3
"""Consumer command fixtures only: no producer admission or installed-package proof."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class NodeDispatchTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="podman-node-dispatch-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.repository = self.directory / "caller with spaces"
        scripts = self.repository / "scripts"
        scripts.mkdir(parents=True)
        for name in ("node-tool.sh", "check-files.sh"):
            shutil.copyfile(ROOT / "scripts" / name, scripts / name)
        # Explicit fixture implementation; this never represents admitted shared code.
        (scripts / "shared-ci.py").write_text(
            "import json, os, pathlib, sys\n"
            "with open(os.environ['DISPATCH_LOG'], 'a') as log:\n"
            " log.write(json.dumps({'args': sys.argv[1:], 'cwd': os.getcwd()}) + '\\n')\n"
            "raise SystemExit(int(os.environ.get('DISPATCH_STATUS', '0')))\n"
        )
        self.bin = self.directory / "bin"
        self.bin.mkdir()
        python = shutil.which("python3")
        assert python is not None
        (self.bin / "python3").write_text(
            f"#!{python}\n"
            "import os, sys\n"
            "if sys.argv[1] == '/opt/podman-lens-file-tools/boxferry-node-tools.py':\n"
            " sys.argv = sys.argv[1:]\n"
            f" exec(open({str(scripts / 'shared-ci.py')!r}).read())\n"
            "else:\n"
            f" os.execv({python!r}, [{python!r}, *sys.argv[1:]])\n"
        )
        (self.bin / "python3").chmod(0o755)
        for name in ("node", "npm", "hadolint", "shellcheck", "shfmt", "tombi"):
            (self.bin / name).symlink_to("/bin/true")
        for name in ("prettier", "markdownlint-cli2"):
            executable = self.bin / name
            executable.write_text("#!/bin/bash\nexit 99\n")
            executable.chmod(0o755)
        git = self.bin / "git"
        git.write_text(
            "#!/bin/bash\n"
            'case "$*" in\n'
            " *'*.md'*) printf '%s\\0' 'README [literal].md' ;;\n"
            " *'*.json'*) printf '%s\\0' 'config.json' ;;\n"
            "esac\n"
        )
        git.chmod(0o755)
        for name in ("README [literal].md", "config.json"):
            (self.repository / name).touch()
        self.log = self.directory / "dispatch.jsonl"
        self.environment = {
            **os.environ,
            "PATH": str(self.bin) + ":/bin",
            "DISPATCH_LOG": str(self.log),
        }
        self.environment.pop("PODMAN_LENS_NODE_TOOLS_ROOT", None)
        self.environment.pop("DISPATCH_STATUS", None)

    def run_checks(self, mode: str = "--check", status: int = 0) -> list[dict]:
        result = subprocess.run(
            ["/bin/bash", str(self.repository / "scripts/check-files.sh"), mode],
            cwd=self.directory,
            env=self.environment,
            capture_output=True,
            text=True,
            check=False,
            timeout=20,
        )
        self.assertEqual(result.returncode, status, result.stdout + result.stderr)
        return [json.loads(line) for line in self.log.read_text().splitlines()]

    def test_check_uses_caller_configs_and_literal_paths_from_outside_root(self) -> None:
        calls = self.run_checks()
        self.assertEqual(len(calls), 3)
        for call in calls:
            self.assertEqual(call["cwd"], str(self.repository))
            self.assertEqual(call["args"][:3], ["node-tool", "--cwd", str(self.repository)])
            self.assertNotIn("--installation-root", call["args"])
        self.assertEqual(
            calls[0]["args"][3:],
            ["markdownlint-cli2", "--config", ".markdownlint.json", ":README [literal].md"],
        )
        self.assertEqual(
            calls[1]["args"][3:],
            [
                "prettier",
                "--check",
                "--config",
                ".prettierrc.json",
                "--no-editorconfig",
                "--ignore-path",
                ".prettierignore",
                "--ignore-unknown",
                "README [literal].md",
            ],
        )
        self.assertEqual(calls[2]["args"][-1], "config.json")

    def test_fix_and_check_share_configuration(self) -> None:
        calls = self.run_checks("--fix")
        calls = calls[1:]  # Existing Python formatting task precedes Node formatting.
        self.assertEqual(len(calls), 6)
        self.assertIn("--write", calls[0]["args"])
        self.assertIn("--fix", calls[1]["args"])
        self.assertIn("--write", calls[2]["args"])
        for call in calls:
            self.assertIn("--config", call["args"])

    def test_image_selection_uses_fixed_readonly_entry_point(self) -> None:
        self.environment["PODMAN_LENS_NODE_TOOLS_ROOT"] = "/opt/podman-lens-file-tools"
        for call in self.run_checks():
            self.assertEqual(
                call["args"][:5],
                [
                    "image-node-tool",
                    "--repository-root",
                    str(self.repository),
                    "--cwd",
                    str(self.repository),
                ],
            )

    def test_unexpected_image_root_is_rejected_before_loading_code(self) -> None:
        self.environment["PODMAN_LENS_NODE_TOOLS_ROOT"] = str(self.directory / "mutable tools")
        result = subprocess.run(
            ["/bin/bash", str(self.repository / "scripts/node-tool.sh"), "prettier", "--version"],
            env=self.environment,
            capture_output=True,
            check=False,
            timeout=20,
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn(b"Unexpected PodmanLens image Node installation", result.stderr)
        self.assertFalse(self.log.exists())

    def test_directory_presence_does_not_select_installation(self) -> None:
        (self.repository / "node_modules/.bin").mkdir(parents=True)
        for call in self.run_checks():
            self.assertNotIn("--installation-root", call["args"])

    def test_image_and_renovate_retain_one_shared_authority(self) -> None:
        marker = json.loads((ROOT / ".github/node-tools.json").read_text())
        self.assertEqual(marker, {"schema": 1, "source": "automation"})
        for name in ("package.json", "package-lock.json"):
            self.assertFalse((ROOT / name).exists())
        image = (ROOT / ".devcontainer/Dockerfile").read_text()
        self.assertIn("--installation-root /opt/podman-lens-file-tools --install-only", image)
        self.assertIn("PODMAN_LENS_NODE_TOOLS_ROOT=/opt/podman-lens-file-tools", image)
        for tool in ("prettier", "markdownlint-cli2"):
            self.assertIn(
                f"ln -s /opt/podman-lens-file-tools/node_modules/.bin/{tool} /usr/local/bin/{tool}",
                image,
            )
        manifest = json.loads((ROOT / ".github/automation.json").read_text())
        renovate = json.loads((ROOT / ".github/renovate.json").read_text())
        self.assertEqual(
            renovate["extends"],
            [
                f"github>Strukturpiloten/boxferry//renovate-presets/{name}#{manifest['revision']}"
                for name in ("common", "rust")
            ],
        )
        self.assertFalse(
            any(
                "markdownlint" in json.dumps(manager) or "prettier" in json.dumps(manager)
                for manager in renovate["customManagers"]
            )
        )

    def test_failure_stops_checks_with_original_exit_status(self) -> None:
        self.environment["DISPATCH_STATUS"] = "17"
        self.assertEqual(len(self.run_checks(status=17)), 1)

    def test_invalid_mode_fails_before_dispatch(self) -> None:
        result = subprocess.run(
            ["/bin/bash", str(self.repository / "scripts/check-files.sh"), "--invalid"],
            env=self.environment,
            capture_output=True,
            check=False,
            timeout=20,
        )
        self.assertEqual(result.returncode, 2)
        self.assertFalse(self.log.exists())


if __name__ == "__main__":
    unittest.main()
