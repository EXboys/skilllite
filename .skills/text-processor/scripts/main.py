#!/usr/bin/env python3
"""
Text Processor - 文本处理工具

此脚本会从 assets/config.json 读取配置，包括：
- defaults.operation: 默认操作类型
- defaults.max_length: 最大文本长度限制
- supported_operations: 支持的操作列表
"""

import json
import os
import sys
import re
from pathlib import Path


def load_config() -> dict:
    """
    从 assets/config.json 加载配置。
    
    通过环境变量 SKILL_ASSETS_DIR 获取 assets 目录路径。
    
    Returns:
        配置字典，如果加载失败则返回默认配置
    """
    default_config = {
        "defaults": {
            "operation": "uppercase",
            "max_length": 100000
        },
        "supported_operations": ["uppercase", "lowercase", "reverse", "trim", "count"]
    }
    
    # 从环境变量获取 assets 目录路径
    assets_dir = os.environ.get("SKILL_ASSETS_DIR")
    if not assets_dir:
        # 如果没有环境变量，尝试相对路径
        script_dir = Path(__file__).parent
        assets_dir = script_dir.parent / "assets"
    else:
        assets_dir = Path(assets_dir)
    
    config_path = assets_dir / "config.json"
    
    if config_path.exists():
        try:
            with open(config_path, "r", encoding="utf-8") as f:
                return json.load(f)
        except (json.JSONDecodeError, IOError):
            pass
    
    return default_config


def normalize_spaces(text: str) -> str:
    """去除多余空格：多个连续空格变成一个，首尾空格去除。"""
    return re.sub(r'\s+', ' ', text).strip()


def process_text(text: str, operation: str, normalize: bool, config: dict) -> dict:
    """
    处理输入的文本。
    
    Args:
        text: 要处理的文本
        operation: 操作类型 (uppercase/lowercase/reverse/trim/count)
        normalize: 是否自动去除多余空格
        config: 配置字典
        
    Returns:
        处理结果字典
    """
    # 检查文本长度限制
    max_length = config.get("defaults", {}).get("max_length", 100000)
    if len(text) > max_length:
        return {
            "success": False,
            "error": f"Text exceeds maximum length of {max_length} characters"
        }
    
    # 检查操作是否支持
    supported_ops = config.get("supported_operations", [])
    if supported_ops and operation not in supported_ops:
        return {
            "success": False,
            "error": f"Unsupported operation: {operation}. Supported: {supported_ops}"
        }
    
    # 先规范化空格（如果启用）
    processed_text = normalize_spaces(text) if normalize else text
    
    result = {
        "success": True,
        "original": text,
        "operation": operation
    }
    
    if operation == "uppercase":
        result["processed"] = processed_text.upper()
    elif operation == "lowercase":
        result["processed"] = processed_text.lower()
    elif operation == "reverse":
        result["processed"] = processed_text[::-1]
    elif operation == "trim":
        result["processed"] = processed_text.strip()
    elif operation == "count":
        result["statistics"] = {
            "length": len(processed_text),
            "words": len(processed_text.split()),
            "lines": processed_text.count('\n') + 1,
            "chars_no_space": len(processed_text.replace(" ", ""))
        }
    else:
        result["success"] = False
        result["error"] = f"Unknown operation: {operation}"
    
    return result


def main():
    """主函数，从 stdin 读取 JSON 输入并输出结果。"""
    try:
        # 加载配置
        config = load_config()
        
        # 读取输入
        input_data = json.loads(sys.stdin.read())
        
        text = input_data.get("text", "")
        # 使用配置中的默认操作类型
        default_operation = config.get("defaults", {}).get("operation", "uppercase")
        operation = input_data.get("operation", default_operation)
        normalize = input_data.get("normalize", True)
        
        if not text:
            result = {
                "success": False,
                "error": "Text is required"
            }
        else:
            result = process_text(text, operation, normalize, config)
        
        print(json.dumps(result, ensure_ascii=False))
        
    except json.JSONDecodeError as e:
        print(json.dumps({
            "success": False,
            "error": f"Invalid JSON input: {e}"
        }))
        sys.exit(1)
    except Exception as e:
        print(json.dumps({
            "success": False,
            "error": str(e)
        }))
        sys.exit(1)


if __name__ == "__main__":
    main()