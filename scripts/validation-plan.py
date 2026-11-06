#!/usr/bin/env python3
"""Compatibility entry point for the immutable shared planner."""

from __future__ import annotations

import runpy
from pathlib import Path

runpy.run_path(str(Path(__file__).with_name("shared-ci.py")), run_name="__main__")
