#!/usr/bin/env python3
"""
Validation script for data-analysis skill
Tests the main functionality of the data analysis skill
"""

import sys
import os
import json

# Add the scripts directory to the path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'scripts'))

# Import the main analysis function
from data_analysis import analyze_data

def test_basic_functionality():
    """Test basic functionality of the data analysis skill"""
    print("=== Testing Data Analysis Skill ===")
    
    # Test 1: Test with sample CSV data
    print("\n1. Testing with sample CSV data (describe):")
    sample_data_path = os.path.join(os.path.dirname(__file__), 'scripts', 'sample_data.csv')
    
    if os.path.exists(sample_data_path):
        result = analyze_data(sample_data_path, "describe")
        if "error" in result:
            print(f"  ❌ Error: {result['error']}")
        else:
            print(f"  ✅ Success! Analysis type: {result.get('analysis_type')}")
            print(f"  Columns analyzed: {list(result.get('result', {}).keys())}")
    else:
        print(f"  ❌ Sample data file not found: {sample_data_path}")
    
    # Test 2: Test with JSON string
    print("\n2. Testing with JSON string (summary):")
    json_data = '[{"name": "Test1", "value": 10}, {"name": "Test2", "value": 20}]'
    result = analyze_data(json_data, "summary")
    if "error" in result:
        print(f"  ❌ Error: {result['error']}")
    else:
        print(f"  ✅ Success! Analysis type: {result.get('analysis_type')}")
    
    # Test 3: Test with Python dict
    print("\n3. Testing with Python dict (aggregate):")
    dict_data = {"scores": [85, 90, 78, 92, 88]}
    result = analyze_data(dict_data, "aggregate", operation="mean")
    if "error" in result:
        print(f"  ❌ Error: {result['error']}")
    else:
        print(f"  ✅ Success! Operation: {result.get('operation')}")
        print(f"  Result: {result.get('result')}")
    
    # Test 4: Test error handling
    print("\n4. Testing error handling (invalid analysis type):")
    result = analyze_data({"test": [1, 2, 3]}, "invalid_type")
    if "error" in result:
        print(f"  ✅ Correctly handled error: {result['error']}")
    else:
        print(f"  ❌ Should have returned an error for invalid analysis type")
    
    return True

def check_skill_structure():
    """Check if the skill has the required structure"""
    print("\n=== Checking Skill Structure ===")
    
    required_files = [
        "SKILL.md",
        "scripts/data_analysis.py",
        "scripts/sample_data.csv"
    ]
    
    all_good = True
    for file_path in required_files:
        full_path = os.path.join(os.path.dirname(__file__), file_path)
        if os.path.exists(full_path):
            print(f"  ✅ {file_path}")
        else:
            print(f"  ❌ {file_path} (missing)")
            all_good = False
    
    # Check directories
    required_dirs = ["scripts", "references", "assets"]
    for dir_name in required_dirs:
        dir_path = os.path.join(os.path.dirname(__file__), dir_name)
        if os.path.isdir(dir_path):
            print(f"  ✅ Directory: {dir_name}/")
        else:
            print(f"  ❌ Directory: {dir_name}/ (missing)")
            all_good = False
    
    return all_good

def check_skill_metadata():
    """Check SKILL.md metadata"""
    print("\n=== Checking Skill Metadata ===")
    
    skill_md_path = os.path.join(os.path.dirname(__file__), "SKILL.md")
    if not os.path.exists(skill_md_path):
        print("  ❌ SKILL.md not found")
        return False
    
    with open(skill_md_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # Check for required sections
    required_sections = [
        "name: data-analysis",
        "description:",
        "license: MIT",
        "# Data Analysis Skill",
        "## 功能特性",
        "## 使用示例",
        "## Runtime 配置"
    ]
    
    all_good = True
    for section in required_sections:
        if section in content:
            print(f"  ✅ Contains: {section}")
        else:
            print(f"  ❌ Missing: {section}")
            all_good = False
    
    return all_good

def main():
    """Main validation function"""
    print("Starting validation of data-analysis skill...")
    
    structure_ok = check_skill_structure()
    metadata_ok = check_skill_metadata()
    
    # Only test functionality if structure is OK
    if structure_ok:
        functionality_ok = test_basic_functionality()
    else:
        print("\n⚠️  Skipping functionality tests due to structure issues")
        functionality_ok = False
    
    # Summary
    print("\n" + "="*50)
    print("VALIDATION SUMMARY:")
    print(f"  Structure: {'✅ PASS' if structure_ok else '❌ FAIL'}")
    print(f"  Metadata:  {'✅ PASS' if metadata_ok else '❌ FAIL'}")
    print(f"  Functionality: {'✅ PASS' if functionality_ok else '❌ FAIL'}")
    
    if structure_ok and metadata_ok and functionality_ok:
        print("\n🎉 Skill validation PASSED!")
        return 0
    else:
        print("\n⚠️  Skill validation FAILED!")
        return 1

if __name__ == "__main__":
    sys.exit(main())