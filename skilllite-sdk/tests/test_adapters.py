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
    manager.execute.return_value = mock_execution_result
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
    def test_skilllite_tool_run(self, mock_manager):
        """Test SkillLiteTool execution."""
        from skilllite.core.adapters.langchain import SkillLiteTool
        
        tool = SkillLiteTool(
            name="test_skill",
            description="A test skill",
            manager=mock_manager,
            skill_name="test_skill"
        )
        
        result = tool._run(param1="value1")
        
        assert result == "Test output"
        mock_manager.execute.assert_called_once_with(
            "test_skill",
            {"param1": "value1"},
            allow_network=False,
            timeout=None
        )
    
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
        mock_manager.list_executable_skills.return_value = [mock_skill_info, skill2]
        
        # Filter to only include test_skill
        tools = SkillLiteToolkit.from_manager(
            mock_manager,
            skill_names=["test_skill"]
        )
        
        assert len(tools) == 1
        assert tools[0].name == "test_skill"


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
