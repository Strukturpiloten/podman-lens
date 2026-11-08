# Script guidance

This scope supplements the [root contract](../AGENTS.md). Use `python3 scripts/validation-plan.py
run-local` for selected checks; the shared locked uv project runs Ruff and ty commands declared in
[validation policy](validation-policy.json). Follow the [testing guide](../docs/testing.md) for
native prerequisites, evidence, budgets, and cleanup. Preserve the Bash native harnesses and the
fixed qualified runtime tuple; do not execute generated plans.

Use `install-host-tools.sh` for the selected shared file-tool inventory, including Actionlint
and Lychee. `install-file-tools.sh` is a forwarding compatibility entry point; do not add
consumer-owned versions or checksums to either launcher. The Dev Container uses the same route.
