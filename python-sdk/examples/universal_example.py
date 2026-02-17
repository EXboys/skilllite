#!/usr/bin/env python3
"""
Example: Using SkillLite with any OpenAI-compatible LLM provider.

This example demonstrates how to use SkillLite with various LLM providers
using the unified OpenAI-compatible API format.

Supported providers:
- OpenAI (GPT-4, GPT-3.5, etc.)
- Azure OpenAI
- Anthropic Claude (via OpenAI-compatible endpoint)
- Google Gemini (via OpenAI-compatible endpoint)
- Local models (Ollama, vLLM, LMStudio, etc.)
- DeepSeek, Qwen, Moonshot, Zhipu, and other providers

Prerequisites:
    pip install skilllite openai
"""

import os
from openai import OpenAI
from skilllite import SkillManager


# ============================================================
# Provider Configuration Examples
# ============================================================

def get_openai_client():
    """Standard OpenAI client."""
    return OpenAI(api_key=os.environ.get("OPENAI_API_KEY"))


def get_azure_openai_client():
    """Azure OpenAI client."""
    from openai import AzureOpenAI
    return AzureOpenAI(
        azure_endpoint=os.environ.get("AZURE_OPENAI_ENDPOINT"),
        api_key=os.environ.get("AZURE_OPENAI_API_KEY"),
        api_version="2024-02-15-preview"
    )


def get_ollama_client():
    """Ollama (local) client."""
    return OpenAI(
        base_url="http://localhost:11434/v1",
        api_key="ollama"  # Ollama doesn't require a real API key
    )


def get_deepseek_client():
    """DeepSeek client."""
    return OpenAI(
        base_url="https://api.deepseek.com/v1",
        api_key=os.environ.get("DEEPSEEK_API_KEY")
    )


def get_qwen_client():
    """Qwen (通义千问) client via DashScope."""
    return OpenAI(
        base_url="https://dashscope.aliyuncs.com/compatible-mode/v1",
        api_key=os.environ.get("DASHSCOPE_API_KEY")
    )


def get_moonshot_client():
    """Moonshot (月之暗面/Kimi) client."""
    return OpenAI(
        base_url="https://api.moonshot.cn/v1",
        api_key=os.environ.get("MOONSHOT_API_KEY")
    )


def get_zhipu_client():
    """Zhipu (智谱) client."""
    return OpenAI(
        base_url="https://open.bigmodel.cn/api/paas/v4",
        api_key=os.environ.get("ZHIPU_API_KEY")
    )


def get_lmstudio_client():
    """LM Studio (local) client."""
    return OpenAI(
        base_url="http://localhost:1234/v1",
        api_key="lm-studio"
    )


def get_vllm_client():
    """vLLM (local) client."""
    return OpenAI(
        base_url="http://localhost:8000/v1",
        api_key="vllm"
    )


# ============================================================
# Main Example
# ============================================================

def main():
    """
    Main example showing how to use SkillLite with any provider.
    """
    # Choose your provider (modify as needed)
    # client = get_openai_client()
    # client = get_ollama_client()
    # client = get_deepseek_client()
    # client = get_qwen_client()
    
    # For this example, we'll try to use OpenAI
    api_key = os.environ.get("OPENAI_API_KEY")
    if not api_key:
        print("No OPENAI_API_KEY found. Trying Ollama...")
        try:
            client = get_ollama_client()
            model = "llama2"  # or any model you have in Ollama
        except Exception:
            print("Ollama not available. Please set up a provider.")
            return
    else:
        client = get_openai_client()
        model = "gpt-4"
    
    # Initialize SkillManager
    skills_dir = os.environ.get("SKILLS_DIR", "./skills")
    
    try:
        manager = SkillManager(skills_dir=skills_dir)
    except FileNotFoundError:
        print(f"Skills directory not found: {skills_dir}")
        print("Please create a skills directory or set SKILLS_DIR environment variable.")
        return
    
    # List available skills
    print("=== Available Skills ===")
    for skill in manager.list_skills():
        print(f"  - {skill.name}: {skill.description or 'No description'}")
    print()
    
    if not manager.list_skills():
        print("No skills found. Please add skills to the directory.")
        return
    
    # Get tools in OpenAI-compatible format
    tools = manager.get_tools()
    print(f"Generated {len(tools)} tool definitions")
    
    # Example conversation
    user_message = "Please help me with a task using the available tools."
    print(f"\nUser: {user_message}")
    print("\nCalling LLM...")
    
    # Call the LLM (works with any OpenAI-compatible provider)
    response = client.chat.completions.create(
        model=model,
        messages=[{"role": "user", "content": user_message}],
        tools=tools if tools else None
    )
    
    message = response.choices[0].message
    print(f"\nLLM response (finish_reason: {response.choices[0].finish_reason}):")
    
    # Handle tool calls if any
    if message.tool_calls:
        print("\nLLM wants to use tools. Executing...")
        
        # Execute all tool calls
        tool_results = manager.handle_tool_calls(response)
        
        for result in tool_results:
            status = "Error" if result.is_error else "Success"
            print(f"  Tool result ({status}): {result.content[:100]}...")
        
        # Build follow-up messages
        messages = [
            {"role": "user", "content": user_message},
            message,  # Assistant message with tool_calls
        ]
        
        # Add tool results
        for result in tool_results:
            messages.append(result.to_openai_format())
        
        # Get final response
        follow_up = client.chat.completions.create(
            model=model,
            messages=messages,
            tools=tools if tools else None
        )
        
        print(f"\nLLM's final response:")
        print(follow_up.choices[0].message.content)
    else:
        # No tool use, just print the response
        print(message.content)


def agentic_loop_example():
    """
    Example using the built-in agentic loop for continuous tool execution.
    
    The agentic loop automatically handles multiple rounds of tool calls
    until the LLM completes its task.
    """
    # Set up client (modify as needed for your provider)
    api_key = os.environ.get("OPENAI_API_KEY")
    if not api_key:
        print("Please set OPENAI_API_KEY or modify this example for your provider.")
        return
    
    client = OpenAI()
    model = "gpt-4"
    
    # Initialize SkillManager
    skills_dir = os.environ.get("SKILLS_DIR", "./skills")
    
    try:
        manager = SkillManager(skills_dir=skills_dir)
    except FileNotFoundError:
        print(f"Skills directory not found: {skills_dir}")
        return
    
    if not manager.list_skills():
        print("No skills found.")
        return
    
    # Create an agentic loop
    loop = manager.create_agentic_loop(
        client=client,
        model=model,
        system_prompt="You are a helpful assistant with access to various skills. Use them to help the user.",
        max_iterations=5,
        temperature=0.7  # Additional kwargs passed to chat.completions.create()
    )
    
    # Run the loop
    user_message = "Please analyze this and provide insights."
    print(f"User: {user_message}")
    print("\nRunning agentic loop...")
    
    final_response = loop.run(user_message)
    
    print("\nFinal response:")
    print(final_response.choices[0].message.content)


if __name__ == "__main__":
    main()
