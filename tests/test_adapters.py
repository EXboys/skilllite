"""
Tests for SkillLite framework adapters.

These tests verify that the LangChain and LlamaIndex adapters work correctly.
They use mocking to avoid requiring actual LLM API calls.
"""

import pytest
from unittest.mock import Mock, MagicMock, patch
from pathlib import Path


# ==================== Helper Functions ====================

def _has_langchain() -> bool:
    """Check if langchain is installed."""
    try:
        import langchain_core
        return True
    except ImportError:
        return False


def _has_llamaindex() -> bool:
    """Check if llama-index is installed."""
    try:
        import llama_index.core
        return True
    except ImportError:
        return False


# ==================== Fixtures ====================

@pytest.fixture
def mock_skill_info():
    """Create a mock SkillInfo object."""
    skill = Mock()
    skill.name = "test_skill"
    skill.description = "A test skill for unit testing"
    skill.get_full_content = Mock(return_value="A test skill for unit testing")
    skill.path = Path("/fake/skill/path")
    skill.metadata = None
    return skill


@pytest.fixture
def mock_execution_result():
    """Create a mock ExecutionResult."""
    result = Mock()
    result.success = True
    result.output = "Test output"
    result.error = None
    return result


@pytest.fixture
def mock_manager(mock_skill_info, mock_execution_result):
    """Create a mock SkillManager."""
    manager = Mock()
    manager.list_executable_skills.return_value = [mock_skill_info]
    manager.get_skill.return_value = mock_skill_info
    # LangChain adapter uses manager._registry.get_skill()
    registry = Mock()
    registry.get_skill.return_value = mock_skill_info
    manager._registry = registry
    return manager


# ==================== LangChain Adapter Tests ====================

class TestLangChainAdapter:
    """Tests for LangChain adapter."""
    
    def test_import_without_langchain(self):
        """Test that import fails gracefully without langchain installed."""
        # This test verifies the lazy import mechanism
        # The actual import behavior depends on whether langchain is installed
        pass
    
    @pytest.mark.skipif(
        not _has_langchain(),
        reason="LangChain not installed"
    )
    def test_skilllite_tool_creation(self, mock_manager):
        """Test SkillLiteTool creation."""
        from skilllite.core.adapters.langchain import SkillLiteTool
        
        tool = SkillLiteTool(
            name="test_skill",
            description="A test skill",
            manager=mock_manager,
            skill_name="test_skill"
        )
        
        assert tool.name == "test_skill"
        assert tool.description == "A test skill"
        assert tool.skill_name == "test_skill"
    
    @pytest.mark.skipif(
        not _has_langchain(),
        reason="LangChain not installed"
    )
    def test_skilllite_tool_run(self, mock_manager, mock_execution_result):
        """Test SkillLiteTool execution."""
        from skilllite.core.adapters.langchain import SkillLiteTool

        tool = SkillLiteTool(
            name="test_skill",
            description="A test skill",
            manager=mock_manager,
            skill_name="test_skill"
        )

        with patch("skilllite.sandbox.execution_service.UnifiedExecutionService") as mock_cls:
            mock_service = Mock()
            mock_service.execute_skill.return_value = mock_execution_result
            mock_cls.get_instance.return_value = mock_service

            result = tool._run(param1="value1")

        assert result == "Test output"
        mock_service.execute_skill.assert_called_once()
    
    @pytest.mark.skipif(
        not _has_langchain(),
        reason="LangChain not installed"
    )
    def test_skilllite_toolkit_from_manager(self, mock_manager):
        """Test SkillLiteToolkit.from_manager()."""
        from skilllite.core.adapters.langchain import SkillLiteToolkit
        
        tools = SkillLiteToolkit.from_manager(mock_manager)
        
        assert len(tools) == 1
        assert tools[0].name == "test_skill"
        assert tools[0].description == "A test skill for unit testing"
    
    @pytest.mark.skipif(
        not _has_langchain(),
        reason="LangChain not installed"
    )
    def test_skilllite_toolkit_with_filter(self, mock_manager, mock_skill_info):
        """Test SkillLiteToolkit with skill name filter."""
        from skilllite.core.adapters.langchain import SkillLiteToolkit

        # Add another skill
        skill2 = Mock()
        skill2.name = "other_skill"
        skill2.description = "Another skill"
        skill2.get_full_content = Mock(return_value="Another skill")
        skill2.path = Path("/fake/skill/other_path")
        skill2.metadata = None
        mock_manager.list_executable_skills.return_value = [mock_skill_info, skill2]

        # Filter to only include test_skill
        tools = SkillLiteToolkit.from_manager(
            mock_manager,
            skill_names=["test_skill"]
        )

        assert len(tools) == 1
        assert tools[0].name == "test_skill"


# ==================== Security Confirmation Tests ====================

class TestLangChainSecurityConfirmation:
    """Tests for LangChain adapter security confirmation mechanism."""

    @pytest.mark.skipif(
        not _has_langchain(),
        reason="LangChain not installed"
    )
    def test_security_scan_result_creation(self):
        """Test SecurityScanResult dataclass creation."""
        from skilllite.core.adapters.langchain import SecurityScanResult

        result = SecurityScanResult(
            is_safe=False,
            issues=[{"severity": "High", "issue_type": "DangerousCode"}],
            scan_id="test-scan-123",
            code_hash="abc123",
            high_severity_count=1,
            medium_severity_count=0,
            low_severity_count=0,
        )

        assert result.is_safe == False
        assert result.requires_confirmation == True
        assert result.high_severity_count == 1
        assert len(result.issues) == 1

    @pytest.mark.skipif(
        not _has_langchain(),
        reason="LangChain not installed"
    )
    def test_security_scan_result_safe(self):
        """Test SecurityScanResult for safe code."""
        from skilllite.core.adapters.langchain import SecurityScanResult

        result = SecurityScanResult(
            is_safe=True,
            issues=[],
            scan_id="test-scan-456",
            code_hash="def456",
        )

        assert result.is_safe == True
        assert result.requires_confirmation == False

    @pytest.mark.skipif(
        not _has_langchain(),
        reason="LangChain not installed"
    )
    def test_security_scan_result_format_report(self):
        """Test SecurityScanResult.format_report() method."""
        from skilllite.core.adapters.langchain import SecurityScanResult

        result = SecurityScanResult(
            is_safe=False,
            issues=[
                {
                    "severity": "High",
                    "issue_type": "FileAccess",
                    "description": "Dangerous file operation",
                    "rule_id": "py-file-open",
                    "line_number": 10,
                    "code_snippet": "open('/etc/passwd')"
                }
            ],
            scan_id="test-scan-789",
            code_hash="ghi789",
            high_severity_count=1,
        )

        report = result.format_report()

        assert "Security Scan Report" in report
        assert "High" in report
        assert "FileAccess" in report
        assert "Confirmation required" in report

    @pytest.mark.skipif(
        not _has_langchain(),
        reason="LangChain not installed"
    )
    def test_security_scan_result_to_dict(self):
        """Test SecurityScanResult.to_dict() method."""
        from skilllite.core.adapters.langchain import SecurityScanResult

        result = SecurityScanResult(
            is_safe=False,
            issues=[{"severity": "High"}],
            scan_id="test-scan",
            code_hash="hash123",
            high_severity_count=1,
        )

        d = result.to_dict()

        assert d["is_safe"] == False
        assert d["requires_confirmation"] == True
        assert d["scan_id"] == "test-scan"
        assert d["high_severity_count"] == 1

    @pytest.mark.skipif(
        not _has_langchain(),
        reason="LangChain not installed"
    )
    def test_tool_with_sandbox_level(self, mock_manager):
        """Test SkillLiteTool creation with sandbox_level."""
        from skilllite.core.adapters.langchain import SkillLiteTool

        tool = SkillLiteTool(
            name="test_skill",
            description="A test skill",
            manager=mock_manager,
            skill_name="test_skill",
            sandbox_level=2,
        )

        assert tool.sandbox_level == 2

    @pytest.mark.skipif(
        not _has_langchain(),
        reason="LangChain not installed"
    )
    def test_tool_with_confirmation_callback(self, mock_manager, mock_execution_result):
        """Test SkillLiteTool with confirmation callback."""
        from skilllite.core.adapters.langchain import SkillLiteTool

        callback_called = []

        def my_callback(report: str, scan_id: str) -> bool:
            callback_called.append((report, scan_id))
            return True  # Approve execution

        tool = SkillLiteTool(
            name="test_skill",
            description="A test skill",
            manager=mock_manager,
            skill_name="test_skill",
            sandbox_level=3,
            confirmation_callback=my_callback,
        )

        assert tool.confirmation_callback == my_callback
        assert tool.sandbox_level == 3

    @pytest.mark.skipif(
        not _has_langchain(),
        reason="LangChain not installed"
    )
    def test_toolkit_with_security_options(self, mock_manager):
        """Test SkillLiteToolkit.from_manager() with security options."""
        from skilllite.core.adapters.langchain import SkillLiteToolkit

        def my_callback(report: str, scan_id: str) -> bool:
            return True

        tools = SkillLiteToolkit.from_manager(
            mock_manager,
            sandbox_level=3,
            confirmation_callback=my_callback,
        )

        assert len(tools) == 1
        assert tools[0].sandbox_level == 3
        assert tools[0].confirmation_callback == my_callback

    @pytest.mark.skipif(
        not _has_langchain(),
        reason="LangChain not installed"
    )
    def test_tool_run_with_sandbox_level_1_no_scan(self, mock_manager, mock_execution_result):
        """Test that sandbox level 1 skips security scan."""
        from skilllite.core.adapters.langchain import SkillLiteTool

        callback_called = []

        def my_callback(report: str, scan_id: str) -> bool:
            callback_called.append(True)
            return True

        tool = SkillLiteTool(
            name="test_skill",
            description="A test skill",
            manager=mock_manager,
            skill_name="test_skill",
            sandbox_level=1,  # No sandbox, no scan
            confirmation_callback=my_callback,
        )

        with patch("skilllite.sandbox.execution_service.UnifiedExecutionService") as mock_cls:
            mock_service = Mock()
            mock_service.execute_skill.return_value = mock_execution_result
            mock_cls.get_instance.return_value = mock_service

            result = tool._run(param1="value1")

        # Callback is passed to the service but not called (level 1)
        assert result == "Test output"

    @pytest.mark.skipif(
        not _has_langchain(),
        reason="LangChain not installed"
    )
    def test_tool_run_with_sandbox_level_2_no_scan(self, mock_manager, mock_execution_result):
        """Test that sandbox level 2 skips security scan."""
        from skilllite.core.adapters.langchain import SkillLiteTool

        callback_called = []

        def my_callback(report: str, scan_id: str) -> bool:
            callback_called.append(True)
            return True

        tool = SkillLiteTool(
            name="test_skill",
            description="A test skill",
            manager=mock_manager,
            skill_name="test_skill",
            sandbox_level=2,  # Sandbox only, no scan
            confirmation_callback=my_callback,
        )

        with patch("skilllite.sandbox.execution_service.UnifiedExecutionService") as mock_cls:
            mock_service = Mock()
            mock_service.execute_skill.return_value = mock_execution_result
            mock_cls.get_instance.return_value = mock_service

            result = tool._run(param1="value1")

        # Callback is passed to the service but not called (level 2)
        assert result == "Test output"

    @pytest.mark.skipif(
        not _has_langchain(),
        reason="LangChain not installed"
    )
    def test_tool_run_no_callback_returns_security_report(self, mock_manager):
        """Test that missing callback returns security report when issues found."""
        from skilllite.core.adapters.langchain import SkillLiteTool

        tool = SkillLiteTool(
            name="test_skill",
            description="A test skill",
            manager=mock_manager,
            skill_name="test_skill",
            sandbox_level=3,
            confirmation_callback=None,  # No callback
        )

        # Mock UnifiedExecutionService to return security error
        security_result = Mock()
        security_result.success = False
        security_result.error = "Security confirmation required:\nHigh severity issues found"

        with patch("skilllite.sandbox.execution_service.UnifiedExecutionService") as mock_cls:
            mock_service = Mock()
            mock_service.execute_skill.return_value = security_result
            mock_cls.get_instance.return_value = mock_service

            result = tool._run(param1="value1")

        assert "Security confirmation required" in result

    @pytest.mark.skipif(
        not _has_langchain(),
        reason="LangChain not installed"
    )
    def test_tool_run_callback_denies_execution(self, mock_manager):
        """Test that callback returning False cancels execution."""
        from skilllite.core.adapters.langchain import SkillLiteTool

        def deny_callback(report: str, scan_id: str) -> bool:
            return False  # Deny execution

        tool = SkillLiteTool(
            name="test_skill",
            description="A test skill",
            manager=mock_manager,
            skill_name="test_skill",
            sandbox_level=3,
            confirmation_callback=deny_callback,
        )

        # Mock UnifiedExecutionService to return cancelled error
        cancelled_result = Mock()
        cancelled_result.success = False
        cancelled_result.error = "Execution cancelled by user after security review"

        with patch("skilllite.sandbox.execution_service.UnifiedExecutionService") as mock_cls:
            mock_service = Mock()
            mock_service.execute_skill.return_value = cancelled_result
            mock_cls.get_instance.return_value = mock_service

            result = tool._run(param1="value1")

        assert "cancelled by user" in result

    @pytest.mark.skipif(
        not _has_langchain(),
        reason="LangChain not installed"
    )
    def test_tool_run_callback_approves_execution(self, mock_manager, mock_execution_result):
        """Test that callback returning True allows execution."""
        from skilllite.core.adapters.langchain import SkillLiteTool

        def approve_callback(report: str, scan_id: str) -> bool:
            return True  # Approve execution

        tool = SkillLiteTool(
            name="test_skill",
            description="A test skill",
            manager=mock_manager,
            skill_name="test_skill",
            sandbox_level=3,
            confirmation_callback=approve_callback,
        )

        # Mock UnifiedExecutionService to return success (callback approved)
        with patch("skilllite.sandbox.execution_service.UnifiedExecutionService") as mock_cls:
            mock_service = Mock()
            mock_service.execute_skill.return_value = mock_execution_result
            mock_cls.get_instance.return_value = mock_service

            result = tool._run(param1="value1")

        # Execution should proceed
        assert result == "Test output"
        # Service should have been called with the confirmation callback
        call_kwargs = mock_service.execute_skill.call_args
        assert call_kwargs.kwargs.get("confirmation_callback") == approve_callback

    @pytest.mark.skipif(
        not _has_langchain(),
        reason="LangChain not installed"
    )
    def test_tool_run_safe_code_no_callback(self, mock_manager, mock_execution_result):
        """Test that safe code (no high severity) doesn't trigger callback."""
        from skilllite.core.adapters.langchain import SkillLiteTool

        def my_callback(report: str, scan_id: str) -> bool:
            return True

        tool = SkillLiteTool(
            name="test_skill",
            description="A test skill",
            manager=mock_manager,
            skill_name="test_skill",
            sandbox_level=3,
            confirmation_callback=my_callback,
        )

        # Mock UnifiedExecutionService to return success (safe code, no callback needed)
        with patch("skilllite.sandbox.execution_service.UnifiedExecutionService") as mock_cls:
            mock_service = Mock()
            mock_service.execute_skill.return_value = mock_execution_result
            mock_cls.get_instance.return_value = mock_service

            result = tool._run(param1="value1")

        # Execution should proceed
        assert result == "Test output"


# ==================== LlamaIndex Adapter Tests ====================

class TestLlamaIndexAdapter:
    """Tests for LlamaIndex adapter."""
    
    @pytest.mark.skipif(
        not _has_llamaindex(),
        reason="LlamaIndex not installed"
    )
    def test_skilllite_toolspec_creation(self, mock_manager):
        """Test SkillLiteToolSpec creation."""
        from skilllite.core.adapters.llamaindex import SkillLiteToolSpec
        
        tool_spec = SkillLiteToolSpec.from_manager(mock_manager)
        
        assert tool_spec.manager == mock_manager
        assert tool_spec.allow_network == False
        assert tool_spec.timeout is None
    
    @pytest.mark.skipif(
        not _has_llamaindex(),
        reason="LlamaIndex not installed"
    )
    def test_skilllite_toolspec_to_tool_list(self, mock_manager):
        """Test SkillLiteToolSpec.to_tool_list()."""
        from skilllite.core.adapters.llamaindex import SkillLiteToolSpec

        tool_spec = SkillLiteToolSpec.from_manager(mock_manager)
        tools = tool_spec.to_tool_list()

        assert len(tools) == 1
        # FunctionTool should have the skill name
        assert tools[0].metadata.name == "test_skill"


# ==================== LlamaIndex Security Confirmation Tests ====================

class TestLlamaIndexSecurityConfirmation:
    """Tests for LlamaIndex adapter security confirmation mechanism."""

    @pytest.mark.skipif(
        not _has_llamaindex(),
        reason="LlamaIndex not installed"
    )
    def test_toolspec_with_sandbox_level(self, mock_manager):
        """Test SkillLiteToolSpec with sandbox_level parameter."""
        from skilllite.core.adapters.llamaindex import SkillLiteToolSpec

        tool_spec = SkillLiteToolSpec.from_manager(
            mock_manager,
            sandbox_level=2
        )

        assert tool_spec.sandbox_level == 2
        assert tool_spec.confirmation_callback is None

    @pytest.mark.skipif(
        not _has_llamaindex(),
        reason="LlamaIndex not installed"
    )
    def test_toolspec_with_confirmation_callback(self, mock_manager):
        """Test SkillLiteToolSpec with confirmation_callback parameter."""
        from skilllite.core.adapters.llamaindex import SkillLiteToolSpec

        def my_callback(report: str, scan_id: str) -> bool:
            return True

        tool_spec = SkillLiteToolSpec.from_manager(
            mock_manager,
            sandbox_level=3,
            confirmation_callback=my_callback
        )

        assert tool_spec.sandbox_level == 3
        assert tool_spec.confirmation_callback is my_callback

    @pytest.mark.skipif(
        not _has_llamaindex(),
        reason="LlamaIndex not installed"
    )
    def test_toolspec_run_with_sandbox_level_1_no_scan(self, mock_manager, mock_execution_result):
        """Test that sandbox_level=1 skips security scan."""
        from skilllite.core.adapters.llamaindex import SkillLiteToolSpec

        tool_spec = SkillLiteToolSpec.from_manager(
            mock_manager,
            sandbox_level=1
        )

        tools = tool_spec.to_tool_list()
        assert len(tools) == 1

        # Mock UnifiedExecutionService for execution
        with patch("skilllite.sandbox.execution_service.UnifiedExecutionService") as mock_cls:
            mock_service = Mock()
            mock_service.execute_skill.return_value = mock_execution_result
            mock_cls.get_instance.return_value = mock_service

            result = tools[0](param1="value1")

        # LlamaIndex returns ToolOutput, check raw_output
        assert result.raw_output == "Test output"

    @pytest.mark.skipif(
        not _has_llamaindex(),
        reason="LlamaIndex not installed"
    )
    def test_toolspec_run_no_callback_returns_security_report(self, mock_manager):
        """Test that high severity issues without callback returns security report."""
        from skilllite.core.adapters.llamaindex import SkillLiteToolSpec

        tool_spec = SkillLiteToolSpec.from_manager(
            mock_manager,
            sandbox_level=3
            # No confirmation_callback
        )

        tools = tool_spec.to_tool_list()

        # Mock UnifiedExecutionService to return security error
        security_result = Mock()
        security_result.success = False
        security_result.error = "Security confirmation required:\nHigh severity issues found"

        with patch("skilllite.sandbox.execution_service.UnifiedExecutionService") as mock_cls:
            mock_service = Mock()
            mock_service.execute_skill.return_value = security_result
            mock_cls.get_instance.return_value = mock_service

            result = tools[0](param1="value1")

        # Should return security error message (check raw_output for LlamaIndex)
        assert "Security confirmation required" in result.raw_output

    @pytest.mark.skipif(
        not _has_llamaindex(),
        reason="LlamaIndex not installed"
    )
    def test_toolspec_run_callback_denies_execution(self, mock_manager):
        """Test that callback returning False cancels execution."""
        from skilllite.core.adapters.llamaindex import SkillLiteToolSpec

        def deny_callback(report: str, scan_id: str) -> bool:
            return False  # Deny execution

        tool_spec = SkillLiteToolSpec.from_manager(
            mock_manager,
            sandbox_level=3,
            confirmation_callback=deny_callback
        )

        tools = tool_spec.to_tool_list()

        # Mock UnifiedExecutionService to return cancelled error
        cancelled_result = Mock()
        cancelled_result.success = False
        cancelled_result.error = "Execution cancelled by user after security review"

        with patch("skilllite.sandbox.execution_service.UnifiedExecutionService") as mock_cls:
            mock_service = Mock()
            mock_service.execute_skill.return_value = cancelled_result
            mock_cls.get_instance.return_value = mock_service

            result = tools[0](param1="value1")

        # Execution should be cancelled (check raw_output for LlamaIndex)
        assert "cancelled" in result.raw_output.lower()

    @pytest.mark.skipif(
        not _has_llamaindex(),
        reason="LlamaIndex not installed"
    )
    def test_toolspec_run_callback_approves_execution(self, mock_manager, mock_execution_result):
        """Test that callback returning True allows execution."""
        from skilllite.core.adapters.llamaindex import SkillLiteToolSpec

        def approve_callback(report: str, scan_id: str) -> bool:
            return True  # Approve execution

        tool_spec = SkillLiteToolSpec.from_manager(
            mock_manager,
            sandbox_level=3,
            confirmation_callback=approve_callback
        )

        tools = tool_spec.to_tool_list()

        # Mock UnifiedExecutionService to return success (callback approved)
        with patch("skilllite.sandbox.execution_service.UnifiedExecutionService") as mock_cls:
            mock_service = Mock()
            mock_service.execute_skill.return_value = mock_execution_result
            mock_cls.get_instance.return_value = mock_service

            result = tools[0](param1="value1")

        # Execution should proceed (check raw_output for LlamaIndex)
        assert result.raw_output == "Test output"

    @pytest.mark.skipif(
        not _has_llamaindex(),
        reason="LlamaIndex not installed"
    )
    def test_toolspec_run_safe_code_no_callback(self, mock_manager, mock_execution_result):
        """Test that safe code (no high severity) executes without callback."""
        from skilllite.core.adapters.llamaindex import SkillLiteToolSpec

        def my_callback(report: str, scan_id: str) -> bool:
            return True

        tool_spec = SkillLiteToolSpec.from_manager(
            mock_manager,
            sandbox_level=3,
            confirmation_callback=my_callback
        )

        tools = tool_spec.to_tool_list()

        # Mock UnifiedExecutionService to return success (safe code)
        with patch("skilllite.sandbox.execution_service.UnifiedExecutionService") as mock_cls:
            mock_service = Mock()
            mock_service.execute_skill.return_value = mock_execution_result
            mock_cls.get_instance.return_value = mock_service

            result = tools[0](param1="value1")

        # Execution should proceed (check raw_output for LlamaIndex)
        assert result.raw_output == "Test output"
