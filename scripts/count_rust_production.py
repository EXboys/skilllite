#!/usr/bin/env python3
"""统计 Rust 生产代码行数：排除专用测试文件与 #[cfg(test)] 块。"""
import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# 明确为“仅测试”的 .rs 文件（整文件不计入生产）
TEST_ONLY_FILES = {
    "crates/skilllite-agent/src/llm/tests.rs",
    "crates/skilllite-agent/src/extensions/builtin/tests.rs",
    "crates/skilllite-sandbox/src/security/dependency_audit/tests.rs",
    "crates/skilllite-sandbox/src/network_proxy/tests.rs",
}


def count_test_block_lines(path: str) -> int:
    """统计 path 中所有 #[cfg(test)] 块的总行数（含 #[cfg(test)] 行与块内行）。"""
    with open(path, "r", encoding="utf-8", errors="replace") as f:
        lines = f.readlines()
    total = 0
    i = 0
    while i < len(lines):
        line = lines[i]
        if "#[cfg(test)]" in line and "cfg(test)" in line:
            # 块从下一行开始（mod tests { 或 fn ...）
            start = i
            i += 1
            depth = 0
            # 找到下一行的开括号
            while i < len(lines):
                l = lines[i]
                for c in l:
                    if c == "{":
                        depth += 1
                    elif c == "}":
                        depth -= 1
                if depth > 0:
                    i += 1
                    continue
                # depth 回到 0，块结束于当前行
                total += i - start + 1
                i += 1
                break
            continue
        i += 1
    return total


def main():
    total_lines = 0
    test_only_lines = 0
    cfg_test_block_lines = 0
    production_lines = 0
    rs_count = 0
    rs_production_count = 0

    for dirpath, _dirnames, filenames in os.walk(ROOT):
        if "target" in dirpath or ".git" in dirpath:
            continue
        for name in filenames:
            if not name.endswith(".rs"):
                continue
            rel = os.path.relpath(os.path.join(dirpath, name), ROOT)
            path = os.path.join(ROOT, rel)
            try:
                with open(path, "r", encoding="utf-8", errors="replace") as f:
                    file_lines = len(f.readlines())
            except Exception:
                continue
            rs_count += 1
            total_lines += file_lines
            if rel.replace("\\", "/") in TEST_ONLY_FILES:
                test_only_lines += file_lines
                continue
            block = count_test_block_lines(path)
            cfg_test_block_lines += block
            production_lines += file_lines - block
            rs_production_count += 1

    print("=== Rust 代码行数统计（正式/生产口径）===")
    print(f"  .rs 文件总数:        {rs_count}")
    print(f"  参与生产统计文件数:  {rs_production_count}（排除 {len(TEST_ONLY_FILES)} 个专用测试文件）")
    print(f"  全部 .rs 总行数:    {total_lines}")
    print(f"  专用测试文件行数:   {test_only_lines}（{TEST_ONLY_FILES})")
    print(f"  #[cfg(test)] 块行数: {cfg_test_block_lines}")
    print(f"  生产代码行数:       {production_lines}（总 - 专用测试 - cfg(test) 块）")
    print()
    print("口径说明：")
    print("  - 生产 = 全部 .rs 行 − 专用测试文件(4 个) − 各文件中 #[cfg(test)] 块内行数")
    print("  - 专用测试文件：llm/tests.rs, extensions/builtin/tests.rs, dependency_audit/tests.rs, network_proxy/tests.rs")


if __name__ == "__main__":
    main()
