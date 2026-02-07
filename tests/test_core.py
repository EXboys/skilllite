"""
Tests for SkillLite core components.

Tests for ExecutionContext, SecurityScanResult, UnifiedExecutionService,
and ToolCallHandler.
"""

import os
import pytest
from unittest.mock import Mock, MagicMock, patch, PropertyMock
from pathlib import Path


# ==================== ExecutionContext Tests ====================

class TestExecutionContext:
    """Tests for ExecutionContext dataclass."""

    def test_default_values(self):
        """Test default context values."""
        from skilllite.sandbox.context import ExecutionContext
        ctx = ExecutionContext()
        assert ctx.sandbox_level == "3"
        assert ctx.allow_network is False
        assert ctx.timeout == 120
        assert ctx.max_memory_mb == 512
        assert ctx.auto_approve is False
        assert ctx.confirmed is False
        assert ctx.scan_id is None
        assert ctx.requires_elevated is False

    def test_from_current_env_defaults(self):
        """Test from_current_env with no env vars set."""
        from skilllite.sandbox.context import ExecutionContext
        env = {
            k: v for k, v in os.environ.items()
            if not k.startswith("SKILLBOX_")
        }
        with patch.dict(os.environ, env, clear=True):
            ctx = ExecutionContext.from_current_env()
        assert ctx.sandbox_level == "3"
        assert ctx.allow_network is False
        assert ctx.timeout == 120
        assert ctx.max_memory_mb == 512

    def test_from_current_env_with_overrides(self):
        """Test from_current_env reads env vars correctly."""
        from skilllite.sandbox.context import ExecutionContext
        env_overrides = {
            "SKILLBOX_SANDBOX_LEVEL": "1",
            "SKILLBOX_ALLOW_NETWORK": "true",
            "SKILLBOX_TIMEOUT_SECS": "60",
            "SKILLBOX_MAX_MEMORY_MB": "256",
            "SKILLBOX_AUTO_APPROVE": "yes",
        }
        with patch.dict(os.environ, env_overrides):
            ctx = ExecutionContext.from_current_env()
        assert ctx.sandbox_level == "1"
        assert ctx.allow_network is True
        assert ctx.timeout == 60
        assert ctx.max_memory_mb == 256
        assert ctx.auto_approve is True

    def test_with_override_partial(self):
        """Test with_override keeps unspecified values."""
        from skilllite.sandbox.context import ExecutionContext
        ctx = ExecutionContext(sandbox_level="3", timeout=120)
        new_ctx = ctx.with_override(timeout=60)
        assert new_ctx.sandbox_level == "3"  # unchanged
        assert new_ctx.timeout == 60  # changed

    def test_with_override_all(self):
        """Test with_override can change all fields."""
        from skilllite.sandbox.context import ExecutionContext
        ctx = ExecutionContext()
        new_ctx = ctx.with_override(
            sandbox_level="1",
            allow_network=True,
            timeout=30,
            max_memory_mb=128,
            auto_approve=True,
            confirmed=True,
            scan_id="test-scan",
            requires_elevated=True,
        )
        assert new_ctx.sandbox_level == "1"
        assert new_ctx.allow_network is True
        assert new_ctx.timeout == 30
        assert new_ctx.max_memory_mb == 128
        assert new_ctx.auto_approve is True
        assert new_ctx.confirmed is True
        assert new_ctx.scan_id == "test-scan"
        assert new_ctx.requires_elevated is True

    def test_with_user_confirmation(self):
        """Test with_user_confirmation downgrades to level 1."""
        from skilllite.sandbox.context import ExecutionContext
        ctx = ExecutionContext(sandbox_level="3")
        confirmed_ctx = ctx.with_user_confirmation("scan-123")
        assert confirmed_ctx.sandbox_level == "1"
        assert confirmed_ctx.confirmed is True
        assert confirmed_ctx.scan_id == "scan-123"

    def test_with_elevated_permissions(self):
        """Test with_elevated_permissions downgrades to level 1."""
        from skilllite.sandbox.context import ExecutionContext
        ctx = ExecutionContext(sandbox_level="3")
        elevated_ctx = ctx.with_elevated_permissions()
        assert elevated_ctx.sandbox_level == "1"
        assert elevated_ctx.requires_elevated is True

    def test_immutability(self):
        """Test that ExecutionContext is frozen (immutable)."""
        from skilllite.sandbox.context import ExecutionContext
        ctx = ExecutionContext()
        with pytest.raises(AttributeError):
            ctx.sandbox_level = "1"

    def test_parse_bool_env_variants(self):
        """Test _parse_bool_env handles various boolean string formats."""
        from skilllite.sandbox.context import ExecutionContext
        for true_val in ["true", "1", "yes", "on", "TRUE", "True"]:
            with patch.dict(os.environ, {"SKILLBOX_ALLOW_NETWORK": true_val}):
                ctx = ExecutionContext.from_current_env()
                assert ctx.allow_network is True, f"Failed for '{true_val}'"
        for false_val in ["false", "0", "no", "off", ""]:
            with patch.dict(os.environ, {"SKILLBOX_ALLOW_NETWORK": false_val}):
                ctx = ExecutionContext.from_current_env()
                assert ctx.allow_network is False, f"Failed for '{false_val}'"


# ==================== SecurityScanResult Tests ====================

class TestSecurityScanResult:
    """Tests for SecurityScanResult dataclass."""

    def test_safe_result(self):
        """Test a safe scan result."""
        from skilllite.core.security import SecurityScanResult
        result = SecurityScanResult(is_safe=True, issues=[], scan_id="s1")
        assert result.is_safe is True
        assert result.requires_confirmation is False

    def test_high_severity_requires_confirmation(self):
        """Test that high severity issues require confirmation."""
        from skilllite.core.security import SecurityScanResult
        result = SecurityScanResult(
            is_safe=False,
            issues=[{"severity": "High", "issue_type": "DangerousCode"}],
            scan_id="s2",
            high_severity_count=1,
        )
        assert result.requires_confirmation is True

    def test_medium_only_no_confirmation(self):
        """Test that medium-only issues don't require confirmation."""
        from skilllite.core.security import SecurityScanResult
        result = SecurityScanResult(
            is_safe=True,
            issues=[{"severity": "Medium", "issue_type": "Warning"}],
            scan_id="s3",
            medium_severity_count=1,
        )
        assert result.requires_confirmation is False

    def test_to_dict(self):
        """Test to_dict serialization."""
        from skilllite.core.security import SecurityScanResult
        result = SecurityScanResult(
            is_safe=False,
            issues=[{"severity": "High"}],
            scan_id="s4",
            code_hash="hash123",
            high_severity_count=1,
            medium_severity_count=2,
            low_severity_count=3,
        )
        d = result.to_dict()
        assert d["is_safe"] is False
        assert d["scan_id"] == "s4"
        assert d["code_hash"] == "hash123"
        assert d["high_severity_count"] == 1
        assert d["medium_severity_count"] == 2
        assert d["low_severity_count"] == 3
        assert d["requires_confirmation"] is True

    def test_format_report_no_issues(self):
        """Test format_report with no issues."""
        from skilllite.core.security import SecurityScanResult
        result = SecurityScanResult(is_safe=True, issues=[], scan_id="s5")
        report = result.format_report()
        assert "No issues found" in report

    def test_format_report_with_issues(self):
        """Test format_report includes issue details."""
        from skilllite.core.security import SecurityScanResult
        result = SecurityScanResult(
            is_safe=False,
            issues=[{
                "severity": "High",
                "issue_type": "FileAccess",
                "description": "Dangerous file operation",
                "rule_id": "py-file-open",
                "line_number": 10,
                "code_snippet": "open('/etc/passwd')",
            }],
            scan_id="s6-full-id",
            high_severity_count=1,
        )
        report = result.format_report()
        assert "Security Scan Report" in report
        assert "High" in report
        assert "FileAccess" in report
        assert "Confirmation required" in report
        assert "s6-full" in report  # scan_id[:8]

    def test_format_report_low_severity_message(self):
        """Test format_report shows safe message for low severity only."""
        from skilllite.core.security import SecurityScanResult
        result = SecurityScanResult(
            is_safe=True,
            issues=[{"severity": "Low", "issue_type": "Minor",
                     "description": "", "rule_id": "", "line_number": 1,
                     "code_snippet": ""}],
            scan_id="s7-abcdef",
            low_severity_count=1,
        )
        report = result.format_report()
        assert "Safe to execute" in report


# ==================== UnifiedExecutionService Tests ====================

class TestUnifiedExecutionService:
    """Tests for UnifiedExecutionService."""

    @pytest.fixture(autouse=True)
    def reset_singleton(self):
        """Reset singleton before each test."""
        from skilllite.sandbox.execution_service import UnifiedExecutionService
        UnifiedExecutionService.reset_instance()
        yield
        UnifiedExecutionService.reset_instance()

    def test_singleton_pattern(self):
        """Test singleton returns same instance."""
        from skilllite.sandbox.execution_service import UnifiedExecutionService
        s1 = UnifiedExecutionService.get_instance()
        s2 = UnifiedExecutionService.get_instance()
        assert s1 is s2

    def test_reset_instance(self):
        """Test reset creates new instance."""
        from skilllite.sandbox.execution_service import UnifiedExecutionService
        s1 = UnifiedExecutionService.get_instance()
        UnifiedExecutionService.reset_instance()
        s2 = UnifiedExecutionService.get_instance()
        assert s1 is not s2

    @patch("skilllite.sandbox.execution_service.UnifiedExecutor")
    def test_execute_skill_level1_no_scan(self, mock_executor_cls):
        """Test that sandbox level 1 skips security scan."""
        from skilllite.sandbox.execution_service import UnifiedExecutionService
        from skilllite.sandbox.base import ExecutionResult

        mock_executor = Mock()
        mock_executor.execute.return_value = ExecutionResult(success=True, output={"result": "ok"})
        mock_executor_cls.return_value = mock_executor

        service = UnifiedExecutionService.get_instance()

        skill_info = Mock()
        skill_info.metadata = None
        skill_info.path = Path("/fake/skill")

        with patch.dict(os.environ, {"SKILLBOX_SANDBOX_LEVEL": "1"}):
            result = service.execute_skill(skill_info, {"input": "data"})

        assert result.success is True
        # Executor should have been called
        mock_executor.execute.assert_called_once()

    @patch("skilllite.sandbox.execution_service.UnifiedExecutor")
    def test_execute_skill_level3_no_issues(self, mock_executor_cls):
        """Test level 3 with safe scan passes through."""
        from skilllite.sandbox.execution_service import UnifiedExecutionService
        from skilllite.sandbox.base import ExecutionResult
        from skilllite.core.security import SecurityScanResult

        mock_executor = Mock()
        mock_executor.execute.return_value = ExecutionResult(success=True, output={"result": "ok"})
        mock_executor_cls.return_value = mock_executor

        service = UnifiedExecutionService.get_instance()
        # Mock scanner to return safe result
        service._scanner = Mock()
        service._scanner.scan_skill.return_value = SecurityScanResult(
            is_safe=True, issues=[], scan_id="safe-scan"
        )

        skill_info = Mock()
        skill_info.metadata = None
        skill_info.path = Path("/fake/skill")

        with patch.dict(os.environ, {"SKILLBOX_SANDBOX_LEVEL": "3"}):
            result = service.execute_skill(skill_info, {"input": "data"})

        assert result.success is True
        mock_executor.execute.assert_called_once()

    @patch("skilllite.sandbox.execution_service.UnifiedExecutor")
    def test_execute_skill_level3_denied(self, mock_executor_cls):
        """Test level 3 with high severity and denied confirmation."""
        from skilllite.sandbox.execution_service import UnifiedExecutionService
        from skilllite.sandbox.base import ExecutionResult
        from skilllite.core.security import SecurityScanResult

        mock_executor = Mock()
        mock_executor_cls.return_value = mock_executor

        service = UnifiedExecutionService.get_instance()
        service._scanner = Mock()
        service._scanner.scan_skill.return_value = SecurityScanResult(
            is_safe=False,
            issues=[{"severity": "High"}],
            scan_id="risky-scan",
            high_severity_count=1,
        )

        skill_info = Mock()
        skill_info.metadata = None
        skill_info.path = Path("/fake/skill")

        def deny_callback(report, scan_id):
            return False

        with patch.dict(os.environ, {"SKILLBOX_SANDBOX_LEVEL": "3"}):
            result = service.execute_skill(
                skill_info, {"input": "data"},
                confirmation_callback=deny_callback,
            )

        assert result.success is False
        assert "cancelled" in result.error.lower()
        mock_executor.execute.assert_not_called()

    @patch("skilllite.sandbox.execution_service.UnifiedExecutor")
    def test_execute_skill_level3_approved(self, mock_executor_cls):
        """Test level 3 with approved confirmation downgrades to level 1."""
        from skilllite.sandbox.execution_service import UnifiedExecutionService
        from skilllite.sandbox.base import ExecutionResult
        from skilllite.core.security import SecurityScanResult

        mock_executor = Mock()
        mock_executor.execute.return_value = ExecutionResult(success=True, output={"result": "ok"})
        mock_executor_cls.return_value = mock_executor

        service = UnifiedExecutionService.get_instance()
        service._scanner = Mock()
        service._scanner.scan_skill.return_value = SecurityScanResult(
            is_safe=False,
            issues=[{"severity": "High"}],
            scan_id="risky-scan",
            high_severity_count=1,
        )

        skill_info = Mock()
        skill_info.metadata = None
        skill_info.path = Path("/fake/skill")

        def approve_callback(report, scan_id):
            return True

        with patch.dict(os.environ, {"SKILLBOX_SANDBOX_LEVEL": "3"}):
            result = service.execute_skill(
                skill_info, {"input": "data"},
                confirmation_callback=approve_callback,
            )

        assert result.success is True
        # Check that executor was called with downgraded context (level 1)
        call_args = mock_executor.execute.call_args
        context_arg = call_args.kwargs.get("context") or call_args[1].get("context") or call_args[0][0]
        assert context_arg.sandbox_level == "1"
        assert context_arg.confirmed is True

    @patch("skilllite.sandbox.execution_service.UnifiedExecutor")
    def test_execute_skill_level3_no_callback(self, mock_executor_cls):
        """Test level 3 with high severity but no callback returns error."""
        from skilllite.sandbox.execution_service import UnifiedExecutionService
        from skilllite.sandbox.base import ExecutionResult
        from skilllite.core.security import SecurityScanResult

        mock_executor = Mock()
        mock_executor_cls.return_value = mock_executor

        service = UnifiedExecutionService.get_instance()
        service._scanner = Mock()
        service._scanner.scan_skill.return_value = SecurityScanResult(
            is_safe=False,
            issues=[{"severity": "High"}],
            scan_id="risky-scan",
            high_severity_count=1,
        )

        skill_info = Mock()
        skill_info.metadata = None
        skill_info.path = Path("/fake/skill")

        with patch.dict(os.environ, {"SKILLBOX_SANDBOX_LEVEL": "3"}):
            result = service.execute_skill(
                skill_info, {"input": "data"},
                confirmation_callback=None,
            )

        assert result.success is False
        assert "Security confirmation required" in result.error
        mock_executor.execute.assert_not_called()

    @patch("skilllite.sandbox.execution_service.UnifiedExecutor")
    def test_execute_skill_elevated_permissions(self, mock_executor_cls):
        """Test skill with elevated permissions downgrades to level 1."""
        from skilllite.sandbox.execution_service import UnifiedExecutionService
        from skilllite.sandbox.base import ExecutionResult

        mock_executor = Mock()
        mock_executor.execute.return_value = ExecutionResult(success=True, output={})
        mock_executor_cls.return_value = mock_executor

        service = UnifiedExecutionService.get_instance()

        skill_info = Mock()
        skill_info.metadata = Mock()
        skill_info.metadata.requires_elevated_permissions = True
        skill_info.path = Path("/fake/skill")

        with patch.dict(os.environ, {"SKILLBOX_SANDBOX_LEVEL": "3"}):
            result = service.execute_skill(skill_info, {})

        assert result.success is True
        call_args = mock_executor.execute.call_args
        context_arg = call_args.kwargs.get("context") or call_args[1].get("context") or call_args[0][0]
        # Elevated permissions should have downgraded to level 1
        assert context_arg.sandbox_level == "1"
        assert context_arg.requires_elevated is True

    def test_temporary_context(self):
        """Test temporary_context restores env vars."""
        from skilllite.sandbox.execution_service import UnifiedExecutionService
        service = UnifiedExecutionService()

        original_level = os.environ.get("SKILLBOX_SANDBOX_LEVEL")
        try:
            os.environ["SKILLBOX_SANDBOX_LEVEL"] = "3"
            with service.temporary_context(sandbox_level="1"):
                assert os.environ["SKILLBOX_SANDBOX_LEVEL"] == "1"
            assert os.environ["SKILLBOX_SANDBOX_LEVEL"] == "3"
        finally:
            if original_level is not None:
                os.environ["SKILLBOX_SANDBOX_LEVEL"] = original_level
            elif "SKILLBOX_SANDBOX_LEVEL" in os.environ:
                del os.environ["SKILLBOX_SANDBOX_LEVEL"]



# ==================== ToolCallHandler Tests ====================

class TestToolCallHandler:
    """Tests for ToolCallHandler."""

    def _make_handler(self, registry=None):
        """Create a ToolCallHandler with a mock registry."""
        from skilllite.core.handler import ToolCallHandler
        if registry is None:
            registry = Mock()
            registry.get_skill.return_value = None
            registry.get_multi_script_tool_info.return_value = None
        return ToolCallHandler(registry=registry)

    def test_execute_skill_not_found(self):
        """Test execute returns error when skill not found."""
        handler = self._make_handler()
        result = handler.execute("nonexistent", {"key": "val"})
        assert result.success is False
        assert "not found" in result.error.lower()

    @patch("skilllite.core.handler.UnifiedExecutionService", create=True)
    def test_execute_regular_skill(self, mock_svc_cls):
        """Test execute delegates to UnifiedExecutionService for regular skill."""
        from skilllite.sandbox.base import ExecutionResult

        mock_service = Mock()
        mock_service.execute_skill.return_value = ExecutionResult(
            success=True, output={"result": "hello"}
        )

        # Patch get_instance at the module level
        with patch("skilllite.sandbox.execution_service.UnifiedExecutionService.get_instance",
                    return_value=mock_service):
            skill_info = Mock()
            skill_info.path = Path("/fake/skill")

            registry = Mock()
            registry.get_multi_script_tool_info.return_value = None
            registry.get_skill.return_value = skill_info

            handler = self._make_handler(registry)
            result = handler.execute("my_skill", {"x": 1})

        assert result.success is True
        assert result.output == {"result": "hello"}
        mock_service.execute_skill.assert_called_once()

    @patch("skilllite.core.handler.UnifiedExecutionService", create=True)
    def test_execute_multi_script_tool(self, mock_svc_cls):
        """Test execute handles multi-script tools correctly."""
        from skilllite.sandbox.base import ExecutionResult

        mock_service = Mock()
        mock_service.execute_skill.return_value = ExecutionResult(
            success=True, output={"step": "done"}
        )

        with patch("skilllite.sandbox.execution_service.UnifiedExecutionService.get_instance",
                    return_value=mock_service):
            parent_skill = Mock()
            parent_skill.path = Path("/fake/parent")

            registry = Mock()
            registry.get_multi_script_tool_info.return_value = {
                "skill_name": "parent",
                "script_path": "scripts/init.py",
            }
            registry.get_skill.return_value = parent_skill

            handler = self._make_handler(registry)
            result = handler.execute("parent__init", {"name": "test"})

        assert result.success is True
        call_kwargs = mock_service.execute_skill.call_args
        assert call_kwargs.kwargs.get("entry_point") == "scripts/init.py"

    def test_execute_tool_call_success(self):
        """Test execute_tool_call wraps result in ToolResult.success."""
        from skilllite.core.tools import ToolUseRequest
        from skilllite.sandbox.base import ExecutionResult

        handler = self._make_handler()
        handler.execute = Mock(return_value=ExecutionResult(
            success=True, output={"data": 42}
        ))

        request = ToolUseRequest(id="call-1", name="calc", input={"a": 1})
        tool_result = handler.execute_tool_call(request)

        assert tool_result.is_error is False
        assert "42" in tool_result.content
        assert tool_result.tool_use_id == "call-1"

    def test_execute_tool_call_failure(self):
        """Test execute_tool_call wraps error in ToolResult.error."""
        from skilllite.core.tools import ToolUseRequest
        from skilllite.sandbox.base import ExecutionResult

        handler = self._make_handler()
        handler.execute = Mock(return_value=ExecutionResult(
            success=False, error="timeout exceeded"
        ))

        request = ToolUseRequest(id="call-2", name="slow", input={})
        tool_result = handler.execute_tool_call(request)

        assert tool_result.is_error is True
        assert "timeout" in tool_result.content.lower()

    def test_handle_tool_calls_openai_format(self):
        """Test handle_tool_calls parses OpenAI response and executes."""
        from skilllite.sandbox.base import ExecutionResult
        import json as json_mod

        handler = self._make_handler()
        handler.execute = Mock(return_value=ExecutionResult(
            success=True, output={"answer": "yes"}
        ))

        # Simulate OpenAI-compatible response
        response = {
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "tc-1",
                        "function": {
                            "name": "check",
                            "arguments": json_mod.dumps({"q": "test"})
                        }
                    }]
                }
            }]
        }

        results = handler.handle_tool_calls(response)
        assert len(results) == 1
        assert results[0].is_error is False
        handler.execute.assert_called_once_with(
            skill_name="check",
            input_data={"q": "test"},
            confirmation_callback=None,
            allow_network=None,
            timeout=None,
        )

    def test_handle_tool_calls_claude_native(self):
        """Test handle_tool_calls_claude_native parses Claude response."""
        from skilllite.sandbox.base import ExecutionResult

        handler = self._make_handler()
        handler.execute = Mock(return_value=ExecutionResult(
            success=True, output={"v": 1}
        ))

        response = {
            "content": [
                {"type": "tool_use", "id": "tu-1", "name": "calc", "input": {"a": 2}},
                {"type": "text", "text": "some text"},
            ]
        }

        results = handler.handle_tool_calls_claude_native(response)
        assert len(results) == 1
        assert results[0].tool_use_id == "tu-1"
        handler.execute.assert_called_once()

    def test_handle_tool_calls_empty(self):
        """Test handle_tool_calls with no tool calls returns empty list."""
        handler = self._make_handler()
        response = {"choices": [{"message": {"tool_calls": None}}]}
        results = handler.handle_tool_calls(response)
        assert results == []
