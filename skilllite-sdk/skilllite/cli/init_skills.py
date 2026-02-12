"""
Skill templates and directory setup for skilllite init.

Creates .skills directory and example skills (hello-world, data-analysis).
"""

from pathlib import Path
from typing import List


# ---------------------------------------------------------------------------
# Hello-world skill template
# ---------------------------------------------------------------------------

HELLO_SKILL_MD = (
    "---\n"
    "name: hello-world\n"
    "description: A simple hello-world skill for testing the SkillLite setup.\n"
    "license: MIT\n"
    "metadata:\n"
    "  author: skilllite-init\n"
    '  version: "1.0"\n'
    "---\n"
    "\n"
    "# Hello World Skill\n"
    "\n"
    "A minimal skill that echoes back a greeting.\n"
    "Use this to verify your SkillLite setup works.\n"
    "\n"
    "## Usage\n"
    "\n"
    "Provide a JSON input with a `name` field:\n"
    "\n"
    '```json\n{"name": "World"}\n```\n'
)

HELLO_MAIN_PY = (
    "#!/usr/bin/env python3\n"
    '"""Hello-world skill entry point."""\n'
    "import json\n"
    "import sys\n"
    "\n"
    "\n"
    "def main():\n"
    "    data = json.loads(sys.stdin.read())\n"
    '    name = data.get("name", "World")\n'
    '    result = {"greeting": f"Hello, {name}!"}\n'
    "    print(json.dumps(result))\n"
    "\n"
    "\n"
    'if __name__ == "__main__":\n'
    "    main()\n"
)

# ---------------------------------------------------------------------------
# Data-analysis skill template (has pandas + numpy dependencies)
# ---------------------------------------------------------------------------

DATA_ANALYSIS_SKILL_MD = (
    "---\n"
    "name: data-analysis\n"
    "description: Analyze CSV/JSON data with statistics, filtering, and aggregation. "
    "Powered by pandas and numpy.\n"
    "compatibility: Requires Python 3.x with pandas, numpy\n"
    "license: MIT\n"
    "metadata:\n"
    "  author: skilllite-init\n"
    '  version: "1.0"\n'
    "---\n"
    "\n"
    "# Data Analysis Skill\n"
    "\n"
    "Perform statistical analysis on tabular data using pandas and numpy.\n"
    "\n"
    "## Supported Operations\n"
    "\n"
    "- **describe**: Summary statistics (mean, std, min, max, etc.)\n"
    "- **filter**: Filter rows by column conditions\n"
    "- **aggregate**: Group-by aggregation (sum, mean, count, etc.)\n"
    "- **correlate**: Correlation matrix between numeric columns\n"
    "\n"
    "## Usage\n"
    "\n"
    '```json\n{"operation": "describe", "data": [[1,2],[3,4]], '
    '"columns": ["a","b"]}\n```\n'
)

DATA_ANALYSIS_MAIN_PY = (
    "#!/usr/bin/env python3\n"
    '"""Data analysis skill entry point."""\n'
    "import json\n"
    "import sys\n"
    "\n"
    "import numpy as np\n"
    "import pandas as pd\n"
    "\n"
    "\n"
    "def main():\n"
    "    data = json.loads(sys.stdin.read())\n"
    '    operation = data.get("operation", "describe")\n'
    '    rows = data.get("data", [])\n'
    '    columns = data.get("columns")\n'
    "\n"
    "    df = pd.DataFrame(rows, columns=columns)\n"
    "\n"
    '    if operation == "describe":\n'
    "        result = json.loads(df.describe().to_json())\n"
    '    elif operation == "filter":\n'
    '        col = data.get("column", df.columns[0])\n'
    '        op = data.get("op", ">")\n'
    '        val = data.get("value", 0)\n'
    '        if op == ">":\n'
    "            filtered = df[df[col] > val]\n"
    '        elif op == "<":\n'
    "            filtered = df[df[col] < val]\n"
    '        elif op == "==":\n'
    "            filtered = df[df[col] == val]\n"
    "        else:\n"
    "            filtered = df\n"
    '        result = {"filtered": json.loads(filtered.to_json(orient="records")),\n'
    '                  "count": len(filtered)}\n'
    '    elif operation == "aggregate":\n'
    '        group_col = data.get("group_by", df.columns[0])\n'
    '        agg_col = data.get("agg_column", df.columns[-1])\n'
    '        agg_func = data.get("agg_func", "mean")\n'
    "        grouped = df.groupby(group_col)[agg_col].agg(agg_func)\n"
    "        result = json.loads(grouped.to_json())\n"
    '    elif operation == "correlate":\n'
    "        numeric = df.select_dtypes(include=[np.number])\n"
    "        result = json.loads(numeric.corr().to_json())\n"
    "    else:\n"
    '        result = {"error": f"Unknown operation: {operation}"}\n'
    "\n"
    "    print(json.dumps(result))\n"
    "\n"
    "\n"
    'if __name__ == "__main__":\n'
    "    main()\n"
)


def create_skills_directory(
    skills_dir: Path,
    skills_dir_rel: str,
    force: bool,
) -> List[str]:
    """Create .skills directory and example skills.

    Returns list of created file paths (relative to project) for summary.
    """
    created_files: List[str] = []

    if not skills_dir.exists():
        skills_dir.mkdir(parents=True, exist_ok=True)
        print(f"\u2713 Created skills directory: {skills_dir_rel}")
    else:
        print(f"\u2713 Skills directory already exists: {skills_dir_rel}")

    hello_dir = skills_dir / "hello-world"
    if not hello_dir.exists() or force:
        hello_dir.mkdir(parents=True, exist_ok=True)
        (hello_dir / "SKILL.md").write_text(HELLO_SKILL_MD, encoding="utf-8")
        scripts_dir = hello_dir / "scripts"
        scripts_dir.mkdir(parents=True, exist_ok=True)
        (scripts_dir / "main.py").write_text(HELLO_MAIN_PY, encoding="utf-8")
        created_files.extend([
            f"{skills_dir_rel}/hello-world/SKILL.md",
            f"{skills_dir_rel}/hello-world/scripts/main.py",
        ])
        print("\u2713 Created hello-world example skill")
    else:
        print("\u2713 hello-world skill already exists (use --force to overwrite)")

    analysis_dir = skills_dir / "data-analysis"
    if not analysis_dir.exists() or force:
        analysis_dir.mkdir(parents=True, exist_ok=True)
        (analysis_dir / "SKILL.md").write_text(DATA_ANALYSIS_SKILL_MD, encoding="utf-8")
        da_scripts = analysis_dir / "scripts"
        da_scripts.mkdir(parents=True, exist_ok=True)
        (da_scripts / "main.py").write_text(DATA_ANALYSIS_MAIN_PY, encoding="utf-8")
        created_files.extend([
            f"{skills_dir_rel}/data-analysis/SKILL.md",
            f"{skills_dir_rel}/data-analysis/scripts/main.py",
        ])
        print("\u2713 Created data-analysis example skill (pandas, numpy)")
    else:
        print("\u2713 data-analysis skill already exists (use --force to overwrite)")

    return created_files
