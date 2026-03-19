"""Smoke tests (no bundled binary required)."""

import skilllite


def test_version_is_defined() -> None:
    assert skilllite.__version__
    assert "." in skilllite.__version__
