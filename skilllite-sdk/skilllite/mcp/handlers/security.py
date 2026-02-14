"""Security scan result and reporting for MCP code execution.

Re-exports SecurityScanResult from the canonical location (core/security.py).
All hard-blocked logic, factory methods, and format_report are defined there.
"""

from ...core.security import SecurityScanResult

__all__ = ["SecurityScanResult"]
