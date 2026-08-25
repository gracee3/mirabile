#!/usr/bin/env python3
"""Lightweight standard-library tests for local CDP and artifact contracts."""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))

from cdp_client import CDPError, require_loopback  # noqa: E402


def load_workbench_control():
    path = SCRIPTS / "workbench-control.py"
    spec = importlib.util.spec_from_file_location("workbench_control", path)
    if spec is None or spec.loader is None:
        raise RuntimeError("could not load workbench-control.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class CDPClientTests(unittest.TestCase):
    def test_only_loopback_endpoints_are_accepted(self) -> None:
        require_loopback("http://127.0.0.1:9222/json/list")
        require_loopback("ws://localhost:9222/devtools/page/1")
        with self.assertRaises(CDPError):
            require_loopback("ws://192.168.1.4:9222/devtools/page/1")

    def test_control_addresses_are_compared_semantically(self) -> None:
        module = load_workbench_control()
        self.assertEqual(
            module.canonical_address(
                {
                    "control": "view.slot",
                    "qualifiers": {"view": "v1", "slot": "primary"},
                }
            ),
            "view.slot[slot=primary,view=v1]",
        )

    def test_cli_has_no_arbitrary_evaluate_command(self) -> None:
        module = load_workbench_control()
        help_text = module.parser().format_help()
        self.assertNotIn("evaluate", help_text)

    def test_failure_artifact_names_are_complete(self) -> None:
        module = load_workbench_control()
        source = Path(module.__file__).read_text(encoding="utf-8")
        for name in (
            "screenshot.png",
            "dom.html",
            "controls.json",
            "application.json",
            "trace.json",
            "browser.log",
            "scenario.json",
            "error.txt",
        ):
            self.assertIn(name, source)

    def test_scenario_expectations_follow_dotted_object_paths(self) -> None:
        module = load_workbench_control()
        module.assert_expectations(
            {"value": {"chart": {"saved": True}}},
            {"value.chart.saved": True},
            3,
        )
        with self.assertRaises(CDPError):
            module.assert_expectations(
                {"value": {"chart": {"saved": False}}},
                {"value.chart.saved": True},
                3,
            )


if __name__ == "__main__":
    unittest.main()
