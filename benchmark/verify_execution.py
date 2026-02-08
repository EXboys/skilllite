#!/usr/bin/env python3
"""
验证 Docker 和 srt 是否正确执行 Python 代码

用于诊断性能测试中代码是否真的被执行
"""

import subprocess
import json
import tempfile
import os
import shutil

def test_srt_execution():
    """测试 srt 是否正确执行 Python 代码"""
    print("=" * 70)
    print("  测试 srt (Claude Code Sandbox) 代码执行")
    print("=" * 70)
    
    # 检查 srt 是否可用
    if not shutil.which("srt"):
        print("❌ srt 未找到，请先安装: npm install -g @anthropic-ai/sandbox-runtime")
        return False
    
    # 检查依赖
    if not shutil.which("rg"):
        print("⚠️  警告: ripgrep (rg) 未找到，srt 需要此依赖")
        print("   安装方法: brew install ripgrep")
        print("   继续测试，但可能会失败...\n")
    
    work_dir = tempfile.mkdtemp(prefix="srt_verify_")
    
    try:
        # 测试 1: 简单 print
        print("\n[测试 1] 简单 print 语句")
        print("-" * 70)
        test_code = 'print("Hello from srt!")'
        script_path = os.path.join(work_dir, "test1.py")
        with open(script_path, "w") as f:
            f.write(test_code)
        
        result = subprocess.run(
            ["srt", "python3", script_path],
            capture_output=True,
            timeout=30,
            cwd=work_dir
        )
        
        print(f"  返回码: {result.returncode}")
        print(f"  标准输出: {result.stdout.decode(errors='replace')[:200]}")
        print(f"  标准错误: {result.stderr.decode(errors='replace')[:200]}")
        
        if result.returncode == 0 and "Hello from srt!" in result.stdout.decode():
            print("  ✓ 测试通过")
        else:
            print("  ✗ 测试失败")
        
        # 测试 2: JSON 输出
        print("\n[测试 2] JSON 输出")
        print("-" * 70)
        test_code = '''
import json
result = {"test": "success", "value": 42}
print(json.dumps(result))
'''
        script_path = os.path.join(work_dir, "test2.py")
        with open(script_path, "w") as f:
            f.write(test_code)
        
        result = subprocess.run(
            ["srt", "python3", script_path],
            capture_output=True,
            timeout=30,
            cwd=work_dir
        )
        
        print(f"  返回码: {result.returncode}")
        stdout = result.stdout.decode(errors='replace')
        print(f"  标准输出: {stdout[:200]}")
        print(f"  标准错误: {result.stderr.decode(errors='replace')[:200]}")
        
        try:
            output_json = json.loads(stdout.strip())
            if output_json.get("test") == "success":
                print("  ✓ 测试通过 - JSON 解析成功")
            else:
                print("  ✗ 测试失败 - JSON 内容不正确")
        except json.JSONDecodeError:
            print("  ✗ 测试失败 - 无法解析 JSON")
        
        # 测试 3: 计算任务
        print("\n[测试 3] 计算任务 (fibonacci)")
        print("-" * 70)
        test_code = '''
import json
def fib(n):
    if n <= 1: return n
    return fib(n-1) + fib(n-2)
result = fib(20)
print(json.dumps({"result": result}))
'''
        script_path = os.path.join(work_dir, "test3.py")
        with open(script_path, "w") as f:
            f.write(test_code)
        
        result = subprocess.run(
            ["srt", "python3", script_path],
            capture_output=True,
            timeout=30,
            cwd=work_dir
        )
        
        print(f"  返回码: {result.returncode}")
        stdout = result.stdout.decode(errors='replace')
        print(f"  标准输出: {stdout[:200]}")
        print(f"  标准错误: {result.stderr.decode(errors='replace')[:200]}")
        
        try:
            output_json = json.loads(stdout.strip())
            if output_json.get("result") == 6765:  # fib(20) = 6765
                print("  ✓ 测试通过 - 计算结果正确")
            else:
                print(f"  ✗ 测试失败 - 期望 6765，得到 {output_json.get('result')}")
        except json.JSONDecodeError:
            print("  ✗ 测试失败 - 无法解析 JSON")
        
        return True
        
    finally:
        shutil.rmtree(work_dir, ignore_errors=True)


def test_docker_execution():
    """测试 Docker 是否正确执行 Python 代码"""
    print("\n" + "=" * 70)
    print("  测试 Docker 代码执行")
    print("=" * 70)
    
    # 检查 Docker 是否可用
    if not shutil.which("docker"):
        print("❌ Docker 未找到")
        return False
    
    try:
        result = subprocess.run(
            ["docker", "version"],
            capture_output=True,
            timeout=10
        )
        if result.returncode != 0:
            print("❌ Docker 不可用")
            return False
    except:
        print("❌ Docker 不可用")
        return False
    
    # 测试 1: 简单 print
    print("\n[测试 1] 简单 print 语句")
    print("-" * 70)
    test_code = 'print("Hello from Docker!")'
    
    result = subprocess.run(
        ["docker", "run", "--rm", "python:3.11-slim", "python", "-c", test_code],
        capture_output=True,
        timeout=60
    )
    
    print(f"  返回码: {result.returncode}")
    print(f"  标准输出: {result.stdout.decode(errors='replace')[:200]}")
    print(f"  标准错误: {result.stderr.decode(errors='replace')[:200]}")
    
    if result.returncode == 0 and "Hello from Docker!" in result.stdout.decode():
        print("  ✓ 测试通过")
    else:
        print("  ✗ 测试失败")
    
    # 测试 2: JSON 输出
    print("\n[测试 2] JSON 输出")
    print("-" * 70)
    test_code = '''
import json
result = {"test": "success", "value": 42}
print(json.dumps(result))
'''
    
    result = subprocess.run(
        ["docker", "run", "--rm", "python:3.11-slim", "python", "-c", test_code],
        capture_output=True,
        timeout=60
    )
    
    print(f"  返回码: {result.returncode}")
    stdout = result.stdout.decode(errors='replace')
    print(f"  标准输出: {stdout[:200]}")
    print(f"  标准错误: {result.stderr.decode(errors='replace')[:200]}")
    
    try:
        output_json = json.loads(stdout.strip())
        if output_json.get("test") == "success":
            print("  ✓ 测试通过 - JSON 解析成功")
        else:
            print("  ✗ 测试失败 - JSON 内容不正确")
    except json.JSONDecodeError:
        print("  ✗ 测试失败 - 无法解析 JSON")
    
    # 测试 3: 计算任务
    print("\n[测试 3] 计算任务 (fibonacci)")
    print("-" * 70)
    test_code = '''
import json
def fib(n):
    if n <= 1: return n
    return fib(n-1) + fib(n-2)
result = fib(20)
print(json.dumps({"result": result}))
'''
    
    result = subprocess.run(
        ["docker", "run", "--rm", "python:3.11-slim", "python", "-c", test_code],
        capture_output=True,
        timeout=60
    )
    
    print(f"  返回码: {result.returncode}")
    stdout = result.stdout.decode(errors='replace')
    print(f"  标准输出: {stdout[:200]}")
    print(f"  标准错误: {result.stderr.decode(errors='replace')[:200]}")
    
    try:
        output_json = json.loads(stdout.strip())
        if output_json.get("result") == 6765:  # fib(20) = 6765
            print("  ✓ 测试通过 - 计算结果正确")
        else:
            print(f"  ✗ 测试失败 - 期望 6765，得到 {output_json.get('result')}")
    except json.JSONDecodeError:
        print("  ✗ 测试失败 - 无法解析 JSON")
    
    return True


def test_skillbox_execution():
    """测试 Skillbox 是否正确执行 Python 代码"""
    print("\n" + "=" * 70)
    print("  测试 Skillbox 代码执行")
    print("=" * 70)
    
    if not shutil.which("skillbox"):
        print("❌ skillbox 未找到")
        return False
    
    work_dir = tempfile.mkdtemp(prefix="skillbox_verify_")
    
    try:
        # 创建 skill 目录结构
        skill_dir = os.path.join(work_dir, "test-skill")
        scripts_dir = os.path.join(skill_dir, "scripts")
        os.makedirs(scripts_dir, exist_ok=True)
        
        with open(os.path.join(skill_dir, "SKILL.md"), "w") as f:
            f.write("---\nname: test\nversion: 1.0.0\nentry_point: scripts/main.py\n---\n")
        
        # 测试 1: 简单 print
        print("\n[测试 1] 简单 print 语句")
        print("-" * 70)
        test_code = 'print("Hello from Skillbox!")'
        script_path = os.path.join(scripts_dir, "main.py")
        with open(script_path, "w") as f:
            f.write(test_code)
        
        result = subprocess.run(
            ["skillbox", "run", skill_dir, "{}"],
            capture_output=True,
            timeout=30,
            cwd=work_dir
        )
        
        print(f"  返回码: {result.returncode}")
        print(f"  标准输出: {result.stdout.decode(errors='replace')[:200]}")
        print(f"  标准错误: {result.stderr.decode(errors='replace')[:200]}")
        
        if result.returncode == 0 and "Hello from Skillbox!" in result.stdout.decode():
            print("  ✓ 测试通过")
        else:
            print("  ✗ 测试失败")
        
        # 测试 2: JSON 输出
        print("\n[测试 2] JSON 输出")
        print("-" * 70)
        test_code = '''
import json
result = {"test": "success", "value": 42}
print(json.dumps(result))
'''
        with open(script_path, "w") as f:
            f.write(test_code)
        
        result = subprocess.run(
            ["skillbox", "run", skill_dir, "{}"],
            capture_output=True,
            timeout=30,
            cwd=work_dir
        )
        
        print(f"  返回码: {result.returncode}")
        stdout = result.stdout.decode(errors='replace')
        print(f"  标准输出: {stdout[:200]}")
        print(f"  标准错误: {result.stderr.decode(errors='replace')[:200]}")
        
        try:
            output_json = json.loads(stdout.strip())
            if output_json.get("test") == "success":
                print("  ✓ 测试通过 - JSON 解析成功")
            else:
                print("  ✗ 测试失败 - JSON 内容不正确")
        except json.JSONDecodeError:
            print("  ✗ 测试失败 - 无法解析 JSON")
        
        # 测试 3: 计算任务
        print("\n[测试 3] 计算任务 (fibonacci)")
        print("-" * 70)
        test_code = '''
import json
def fib(n):
    if n <= 1: return n
    return fib(n-1) + fib(n-2)
result = fib(20)
print(json.dumps({"result": result}))
'''
        with open(script_path, "w") as f:
            f.write(test_code)
        
        result = subprocess.run(
            ["skillbox", "run", skill_dir, "{}"],
            capture_output=True,
            timeout=30,
            cwd=work_dir
        )
        
        print(f"  返回码: {result.returncode}")
        stdout = result.stdout.decode(errors='replace')
        print(f"  标准输出: {stdout[:200]}")
        print(f"  标准错误: {result.stderr.decode(errors='replace')[:200]}")
        
        try:
            output_json = json.loads(stdout.strip())
            if output_json.get("result") == 6765:  # fib(20) = 6765
                print("  ✓ 测试通过 - 计算结果正确")
            else:
                print(f"  ✗ 测试失败 - 期望 6765，得到 {output_json.get('result')}")
        except json.JSONDecodeError:
            print("  ✗ 测试失败 - 无法解析 JSON")
        
        return True
        
    finally:
        shutil.rmtree(work_dir, ignore_errors=True)


if __name__ == "__main__":
    print("\n" + "=" * 70)
    print("  代码执行验证工具")
    print("  用于验证 Docker、srt 和 Skillbox 是否正确执行 Python 代码")
    print("=" * 70)
    
    test_srt_execution()
    test_docker_execution()
    test_skillbox_execution()
    
    print("\n" + "=" * 70)
    print("  验证完成")
    print("=" * 70)
