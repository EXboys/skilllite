"""
Handler - LLM response handling and tool call execution.

This module handles:
- Parsing tool calls from LLM responses
- Executing tool calls
- Formatting tool results
"""

import json
from typing import Any, Dict, List, Optional, TYPE_CHECKING

from .executor import ExecutionResult, SkillExecutor
from .tools import ToolResult, ToolUseRequest

if TYPE_CHECKING:
    from .registry import SkillRegistry


class ToolCallHandler:
    """
    Handler for LLM tool calls.
    
    Parses tool calls from LLM responses and executes them
    using the skill executor.
    """
    
    def __init__(
        self,
        registry: "SkillRegistry",
        executor: SkillExecutor
    ):
        """
        Initialize the handler.
        
        Args:
            registry: Skill registry for accessing skill info
            executor: Skill executor for running skills
        """
        self._registry = registry
        self._executor = executor
    
    # ==================== Skill Execution ====================
    
    def execute(
        self,
        skill_name: str,
        input_data: Dict[str, Any],
        allow_network: Optional[bool] = None,
        timeout: Optional[int] = None
    ) -> ExecutionResult:
        """
        Execute a skill or multi-script tool with the given input.

        Args:
            skill_name: Name of the skill or multi-script tool
                       (e.g., "calculator" or "skill-creator__init-skill")
            input_data: Input data for the skill
            allow_network: Whether to allow network access
            timeout: Execution timeout in seconds

        Returns:
            ExecutionResult with output or error
        """
        # Check if it's a multi-script tool
        tool_info = self._registry.get_multi_script_tool_info(skill_name)
        if tool_info:
            parent_skill = self._registry.get_skill(tool_info["skill_name"])
            if not parent_skill:
                return ExecutionResult(
                    success=False,
                    error=f"Parent skill not found: {tool_info['skill_name']}"
                )
            
            # Execute with the specific script as entry point
            return self._executor.execute(
                skill_dir=parent_skill.path,
                input_data=input_data,
                allow_network=allow_network,
                timeout=timeout,
                entry_point=tool_info["script_path"]
            )
        
        # Regular skill execution
        info = self._registry.get_skill(skill_name)
        if not info:
            return ExecutionResult(
                success=False,
                error=f"Skill not found: {skill_name}"
            )
        
        return self._executor.execute(
            skill_dir=info.path,
            input_data=input_data,
            allow_network=allow_network,
            timeout=timeout
        )
    
    def execute_tool_call(
        self,
        request: ToolUseRequest,
        allow_network: Optional[bool] = None,
        timeout: Optional[int] = None
    ) -> ToolResult:
        """
        Execute a tool call request from an LLM.
        
        Args:
            request: Tool use request from LLM
            allow_network: Whether to allow network access
            timeout: Execution timeout in seconds
            
        Returns:
            ToolResult with success or error
        """
        result = self.execute(
            skill_name=request.name,
            input_data=request.input,
            allow_network=allow_network,
            timeout=timeout
        )
        
        if result.success:
            return ToolResult.success(
                tool_use_id=request.id,
                content=result.output
            )
        else:
            return ToolResult.error(
                tool_use_id=request.id,
                error=result.error or "Unknown error"
            )
    
    # ==================== LLM Response Parsing ====================
    
    def parse_tool_calls(self, response: Any) -> List[ToolUseRequest]:
        """
        Parse tool calls from an OpenAI-compatible LLM response.
        
        Args:
            response: Response from OpenAI-compatible API
            
        Returns:
            List of ToolUseRequest objects
        """
        return ToolUseRequest.parse_from_openai_response(response)
    
    def parse_tool_calls_claude_native(self, response: Any) -> List[ToolUseRequest]:
        """
        Parse tool calls from Claude's native API response.
        
        Args:
            response: Response from Claude's native API
            
        Returns:
            List of ToolUseRequest objects
        """
        return ToolUseRequest.parse_from_claude_response(response)
    
    # ==================== Batch Handling ====================
    
    def handle_tool_calls(
        self,
        response: Any,
        allow_network: Optional[bool] = None,
        timeout: Optional[int] = None
    ) -> List[ToolResult]:
        """
        Parse and execute all tool calls from an OpenAI-compatible LLM response.
        
        Args:
            response: Response from OpenAI-compatible API
            allow_network: Whether to allow network access
            timeout: Execution timeout in seconds
            
        Returns:
            List of ToolResult objects
        """
        requests = self.parse_tool_calls(response)
        results = []
        for request in requests:
            result = self.execute_tool_call(
                request,
                allow_network=allow_network,
                timeout=timeout
            )
            results.append(result)
        return results
    
    def handle_tool_calls_claude_native(
        self,
        response: Any,
        allow_network: Optional[bool] = None,
        timeout: Optional[int] = None
    ) -> List[ToolResult]:
        """
        Parse and execute all tool calls from Claude's native API response.
        
        Args:
            response: Response from Claude's native API
            allow_network: Whether to allow network access
            timeout: Execution timeout in seconds
            
        Returns:
            List of ToolResult objects
        """
        requests = self.parse_tool_calls_claude_native(response)
        results = []
        for request in requests:
            result = self.execute_tool_call(
                request,
                allow_network=allow_network,
                timeout=timeout
            )
            results.append(result)
        return results
    
    def format_tool_results_claude_native(
        self,
        results: List[ToolResult]
    ) -> List[Dict[str, Any]]:
        """
        Format tool results for Claude's native API.
        
        Args:
            results: List of ToolResult objects
            
        Returns:
            List of formatted tool result dicts
        """
        return [r.to_claude_format() for r in results]
    
    # ==================== Enhanced Workflow ====================
    
    def create_enhanced_skill_workflow(
        self,
        skill_name: str,
        user_request: str,
        llm_client: Any,
        llm_model: str = "gpt-4"
    ) -> Dict[str, Any]:
        """
        Create an enhanced workflow for a skill that involves planning and execution.
        
        Implements a two-stage process:
        1. Read skill information and create an execution plan
        2. Execute the plan step by step
        
        Args:
            skill_name: Name of the skill to use
            user_request: User's request or requirements
            llm_client: OpenAI-compatible client for LLM interaction
            llm_model: Model name to use for planning
            
        Returns:
            Dictionary with execution results and status
        """
        skill_info = self._registry.get_skill(skill_name)
        if not skill_info:
            return {"success": False, "error": f"Skill '{skill_name}' not found"}
        
        skill_context = skill_info.get_context(
            include_references=True,
            include_assets=True
        )
        if not skill_context:
            return {
                "success": False,
                "error": f"Failed to get context for skill '{skill_name}'"
            }
        
        plan_prompt = f"""
You are tasked with creating an execution plan for the following skill:

Skill Name: {skill_name}
Skill Description: {skill_info.description or 'No description available'}

Skill Content:
{skill_context.get('full_instructions', 'No instructions available')}

User Request: {user_request}

Please create a detailed execution plan that includes:
1. Analysis of what needs to be done based on the user request
2. Steps to accomplish the task using this skill
3. Expected outcomes
4. Any potential challenges or considerations

Respond in JSON format with the following structure:
{{
  "analysis": "Brief analysis of the request and skill compatibility",
  "steps": [
    {{
      "step_number": 1,
      "description": "What to do in this step",
      "expected_input": "What input is needed",
      "expected_output": "What output is expected"
    }}
  ],
  "considerations": ["Any important points to consider"]
}}
"""
        
        try:
            plan_response = llm_client.chat.completions.create(
                model=llm_model,
                messages=[{"role": "user", "content": plan_prompt}],
                response_format={"type": "json_object"}
            )
            
            plan = json.loads(plan_response.choices[0].message.content)
            
            execution_results = []
            for step in plan.get("steps", []):
                execution_results.append({
                    "step": step["step_number"],
                    "status": "planned",
                    "description": step["description"]
                })
            
            return {
                "success": True,
                "skill_name": skill_name,
                "plan": plan,
                "execution_results": execution_results
            }
            
        except Exception as e:
            return {
                "success": False,
                "error": f"Failed to create execution plan: {str(e)}"
            }
