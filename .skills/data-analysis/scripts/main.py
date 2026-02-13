#!/usr/bin/env python3
"""Data analysis skill entry point."""
import json
import sys

import numpy as np
import pandas as pd


def _normalize_data_shape(rows, columns):
    """Normalize data when columns count doesn't match row length.
    LLM often passes [[v1,v2,v3,...]] with columns=[name] meaning one column of values."""
    if not rows or not columns:
        return rows, columns
    first = rows[0]
    n_cols = len(columns)
    # Single row with many values but only 1 column name -> reshape to column vector
    if n_cols == 1 and isinstance(first, (list, tuple)) and len(first) > 1:
        return [[v] for v in first], columns
    # Flat list [v1,v2,...] with 1 column -> reshape to rows
    if n_cols == 1 and not isinstance(first, (list, tuple)):
        return [[v] for v in rows], columns
    return rows, columns


def main():
    data = json.loads(sys.stdin.read())
    operation = data.get("operation", "describe")
    rows = data.get("data", [])
    columns = data.get("columns")

    rows, columns = _normalize_data_shape(rows, columns)
    df = pd.DataFrame(rows, columns=columns)

    if operation == "describe":
        result = json.loads(df.describe().to_json())
    elif operation == "filter":
        col = data.get("column", df.columns[0])
        op = data.get("op", ">")
        val = data.get("value", 0)
        if op == ">":
            filtered = df[df[col] > val]
        elif op == "<":
            filtered = df[df[col] < val]
        elif op == "==":
            filtered = df[df[col] == val]
        else:
            filtered = df
        result = {"filtered": json.loads(filtered.to_json(orient="records")),
                  "count": len(filtered)}
    elif operation == "aggregate":
        group_col = data.get("group_by", df.columns[0])
        agg_col = data.get("agg_column", df.columns[-1])
        agg_func = data.get("agg_func", "mean")
        grouped = df.groupby(group_col)[agg_col].agg(agg_func)
        result = json.loads(grouped.to_json())
    elif operation == "correlate":
        numeric = df.select_dtypes(include=[np.number])
        result = json.loads(numeric.corr().to_json())
    else:
        result = {"error": f"Unknown operation: {operation}"}

    print(json.dumps(result))


if __name__ == "__main__":
    main()
