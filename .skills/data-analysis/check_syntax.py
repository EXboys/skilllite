#!/usr/bin/env python3
"""
Simple syntax check for the data analysis script
"""

import ast
import sys
import os

def check_python_syntax(file_path):
    """Check if a Python file has valid syntax"""
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # Parse the Python code
        ast.parse(content)
        print(f"✅ {file_path} - Syntax is valid")
        return True
    except SyntaxError as e:
        print(f"❌ {file_path} - Syntax error: {e}")
        return False
    except Exception as e:
        print(f"❌ {file_path} - Error reading file: {e}")
        return False

def main():
    """Main function"""
    print("Checking syntax of data-analysis skill files...")
    
    # Check main script
    main_script = os.path.join(os.path.dirname(__file__), 'scripts', 'data_analysis.py')
    main_ok = check_python_syntax(main_script)
    
    # Check validation script
    validation_script = os.path.join(os.path.dirname(__file__), 'test_validation.py')
    validation_ok = check_python_syntax(validation_script)
    
    # Check SKILL.md structure
    skill_md = os.path.join(os.path.dirname(__file__), 'SKILL.md')
    if os.path.exists(skill_md):
        with open(skill_md, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # Check for required metadata
        required_metadata = ['name:', 'description:', 'license:', 'metadata:']
        missing = []
        for meta in required_metadata:
            if meta not in content:
                missing.append(meta)
        
        if missing:
            print(f"❌ SKILL.md - Missing metadata: {', '.join(missing)}")
            skill_ok = False
        else:
            print("✅ SKILL.md - Contains required metadata")
            skill_ok = True
    else:
        print("❌ SKILL.md - File not found")
        skill_ok = False
    
    # Summary
    print("\n" + "="*50)
    print("SYNTAX CHECK SUMMARY:")
    print(f"  Main script: {'✅ PASS' if main_ok else '❌ FAIL'}")
    print(f"  Validation script: {'✅ PASS' if validation_ok else '❌ FAIL'}")
    print(f"  SKILL.md: {'✅ PASS' if skill_ok else '❌ FAIL'}")
    
    if main_ok and validation_ok and skill_ok:
        print("\n🎉 All syntax checks passed!")
        return 0
    else:
        print("\n⚠️  Some syntax checks failed!")
        return 1

if __name__ == "__main__":
    sys.exit(main())