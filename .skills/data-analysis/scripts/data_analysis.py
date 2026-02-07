#!/usr/bin/env python3
"""
Data Analysis Skill - Main Script

This script provides data analysis capabilities using pandas.
It can perform various statistical operations on data.
"""

import sys
import json
import pandas as pd
import numpy as np
from typing import Dict, Any, Optional, List, Union

def analyze_data(data: Union[str, Dict, List], analysis_type: str = "describe", 
                columns: Optional[List[str]] = None, 
                group_by: Optional[str] = None,
                operation: Optional[str] = None) -> Dict[str, Any]:
    """
    Perform data analysis on the provided data.
    
    Args:
        data: Input data - can be a file path (CSV/JSON), JSON string, or dict/list
        analysis_type: Type of analysis to perform:
            - "describe": Descriptive statistics
            - "summary": Basic summary (count, mean, std, min, max)
            - "aggregate": Aggregate operations (sum, mean, count, etc.)
            - "correlation": Correlation matrix
            - "group": Group by operations
        columns: Specific columns to analyze (if None, analyze all)
        group_by: Column to group by (for group analysis)
        operation: Specific operation for aggregate/group analysis (sum, mean, count, etc.)
    
    Returns:
        Dictionary with analysis results
    """
    try:
        # Load data based on input type
        df = load_data(data)
        
        # Select specific columns if provided
        if columns:
            df = df[columns]
        
        # Perform analysis based on type
        if analysis_type == "describe":
            result = df.describe(include='all').to_dict()
            return {"analysis_type": "describe", "result": result}
        
        elif analysis_type == "summary":
            summary = {
                "count": df.count().to_dict(),
                "mean": df.mean(numeric_only=True).to_dict(),
                "std": df.std(numeric_only=True).to_dict(),
                "min": df.min(numeric_only=True).to_dict(),
                "max": df.max(numeric_only=True).to_dict(),
                "data_types": df.dtypes.astype(str).to_dict()
            }
            return {"analysis_type": "summary", "result": summary}
        
        elif analysis_type == "aggregate":
            if not operation:
                operation = "sum"
            
            if operation == "sum":
                result = df.sum(numeric_only=True).to_dict()
            elif operation == "mean":
                result = df.mean(numeric_only=True).to_dict()
            elif operation == "count":
                result = df.count().to_dict()
            elif operation == "median":
                result = df.median(numeric_only=True).to_dict()
            elif operation == "std":
                result = df.std(numeric_only=True).to_dict()
            else:
                raise ValueError(f"Unsupported operation: {operation}")
            
            return {"analysis_type": "aggregate", "operation": operation, "result": result}
        
        elif analysis_type == "correlation":
            # Calculate correlation matrix for numeric columns
            numeric_df = df.select_dtypes(include=[np.number])
            if numeric_df.empty:
                return {"analysis_type": "correlation", "error": "No numeric columns found"}
            
            correlation_matrix = numeric_df.corr().to_dict()
            return {"analysis_type": "correlation", "result": correlation_matrix}
        
        elif analysis_type == "group":
            if not group_by:
                raise ValueError("group_by parameter is required for group analysis")
            
            if not operation:
                operation = "mean"
            
            if group_by not in df.columns:
                raise ValueError(f"Column '{group_by}' not found in data")
            
            # Group by the specified column
            grouped = df.groupby(group_by)
            
            if operation == "mean":
                result = grouped.mean(numeric_only=True).to_dict()
            elif operation == "sum":
                result = grouped.sum(numeric_only=True).to_dict()
            elif operation == "count":
                result = grouped.count().to_dict()
            elif operation == "size":
                result = grouped.size().to_dict()
            else:
                raise ValueError(f"Unsupported operation for grouping: {operation}")
            
            return {"analysis_type": "group", "group_by": group_by, "operation": operation, "result": result}
        
        else:
            raise ValueError(f"Unsupported analysis type: {analysis_type}")
    
    except Exception as e:
        return {"error": str(e), "analysis_type": analysis_type}

def load_data(data: Union[str, Dict, List]) -> pd.DataFrame:
    """
    Load data from various sources into a pandas DataFrame.
    
    Args:
        data: Input data - can be:
            - File path (CSV or JSON)
            - JSON string
            - Python dict or list
    
    Returns:
        pandas DataFrame
    """
    if isinstance(data, str):
        # Check if it's a file path
        if data.endswith('.csv'):
            return pd.read_csv(data)
        elif data.endswith('.json'):
            return pd.read_json(data)
        else:
            # Try to parse as JSON string
            try:
                data_dict = json.loads(data)
                return pd.DataFrame(data_dict)
            except json.JSONDecodeError:
                # If not JSON, treat as single column data
                return pd.DataFrame({"data": [data]})
    
    elif isinstance(data, dict):
        # If dict, convert to DataFrame
        return pd.DataFrame(data)
    
    elif isinstance(data, list):
        # If list, convert to DataFrame
        return pd.DataFrame(data)
    
    else:
        raise ValueError(f"Unsupported data type: {type(data)}")

def main():
    """Main function for command-line usage"""
    if len(sys.argv) < 2:
        print("Usage: python data_analysis.py <data_file> [analysis_type] [columns...]")
        print("\nExamples:")
        print("  python data_analysis.py data.csv describe")
        print("  python data_analysis.py data.json summary")
        print("  python data_analysis.py '{\"a\": [1,2,3], \"b\": [4,5,6]}' aggregate sum")
        sys.exit(1)
    
    data_input = sys.argv[1]
    analysis_type = sys.argv[2] if len(sys.argv) > 2 else "describe"
    columns = sys.argv[3:] if len(sys.argv) > 3 else None
    
    result = analyze_data(data_input, analysis_type, columns)
    print(json.dumps(result, indent=2, default=str))

if __name__ == "__main__":
    main()