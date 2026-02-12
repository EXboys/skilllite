---
name: data-analysis
description: Analyze CSV/JSON data with statistics, filtering, and aggregation. Powered by pandas and numpy.
compatibility: Requires Python 3.x with pandas, numpy
license: MIT
metadata:
  author: skilllite-init
  version: "1.0"
---

# Data Analysis Skill

Perform statistical analysis on tabular data using pandas and numpy.

## Supported Operations

- **describe**: Summary statistics (mean, std, min, max, etc.)
- **filter**: Filter rows by column conditions
- **aggregate**: Group-by aggregation (sum, mean, count, etc.)
- **correlate**: Correlation matrix between numeric columns

## Usage

```json
{"operation": "describe", "data": [[1,2],[3,4]], "columns": ["a","b"]}
```
