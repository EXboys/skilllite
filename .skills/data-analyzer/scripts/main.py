#!/usr/bin/env python3
"""
数据分析技能 - 使用 pandas 进行数据统计分析
依赖: pandas, numpy (通过沙箱自动安装)
"""

import json
import sys

import pandas as pd


def analyze_data(data_json: str, operation: str = "describe") -> dict:
    """
    分析数据并返回统计结果
    
    Args:
        data_json: JSON 格式的数据字符串
        operation: 分析操作类型 (describe/mean/sum/count)
    
    Returns:
        包含分析结果的字典
    """
    try:
        data = json.loads(data_json)
        df = pd.DataFrame(data)
        
        if operation == "describe":
            result = df.describe(include='all').to_dict()
        elif operation == "mean":
            numeric_cols = df.select_dtypes(include=['number'])
            result = numeric_cols.mean().to_dict()
        elif operation == "sum":
            numeric_cols = df.select_dtypes(include=['number'])
            result = numeric_cols.sum().to_dict()
        elif operation == "count":
            result = {"total_rows": len(df), "columns": df.columns.tolist()}
        else:
            return {"error": f"不支持的操作类型: {operation}"}
        
        return {
            "success": True,
            "operation": operation,
            "result": result,
            "shape": {"rows": df.shape[0], "columns": df.shape[1]}
        }
        
    except json.JSONDecodeError as e:
        return {"success": False, "error": f"JSON 解析错误: {str(e)}"}
    except Exception as e:
        return {"success": False, "error": f"分析错误: {str(e)}"}


def main():
    # Read input from stdin (framework passes JSON via stdin)
    input_data = json.loads(sys.stdin.read())
    
    data_json = input_data.get("data", "[]")
    operation = input_data.get("operation", "describe")
    
    result = analyze_data(data_json, operation)
    # 处理 NaN 值，将其转换为 null
    import math
    def clean_nan(obj):
        if isinstance(obj, dict):
            return {k: clean_nan(v) for k, v in obj.items()}
        elif isinstance(obj, list):
            return [clean_nan(v) for v in obj]
        elif isinstance(obj, float) and math.isnan(obj):
            return None
        return obj
    
    result = clean_nan(result)
    print(json.dumps(result, ensure_ascii=False, default=str))


if __name__ == "__main__":
    main()
