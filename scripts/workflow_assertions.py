"""Deterministic assertions and parity normalization for live-workflow evals."""

from __future__ import annotations

from typing import Any


VOLATILE_KEYS = {
    "created_at",
    "modified_at",
    "projection",
    "revision",
    "sequence",
    "settled_projection",
    "accepted_projection",
    "source",
}
IDENTITY_KEYS = {
    "active_chart",
    "active_view",
    "chart",
    "definition_id",
    "draft_chart",
    "durable_chart",
    "instance_id",
    "resource_id",
    "view_id",
    "workspace_id",
}


def approximately(actual: float, expected: float, tolerance: float) -> None:
    if abs(actual - expected) > tolerance:
        raise AssertionError(f"{actual} is not within {tolerance} of {expected}")


def circularly_approximately(actual: float, expected: float, tolerance: float) -> None:
    difference = abs((actual - expected + 180.0) % 360.0 - 180.0)
    if difference > tolerance:
        raise AssertionError(
            f"circular difference {difference} exceeds {tolerance}: {actual} vs {expected}"
        )


def normalize_parity(value: Any) -> Any:
    """Remove generated identity/clock/trace fields and canonicalize object ordering."""
    if isinstance(value, dict):
        return {
            key: normalize_parity(item)
            for key, item in sorted(value.items())
            if key not in VOLATILE_KEYS and key not in IDENTITY_KEYS
        }
    if isinstance(value, list):
        normalized = [normalize_parity(item) for item in value]
        if all(isinstance(item, dict) for item in normalized):
            return sorted(normalized, key=repr)
        return normalized
    return value
