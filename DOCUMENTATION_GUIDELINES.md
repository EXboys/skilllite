# Documentation Guidelines / 文档规范指南

This document outlines the documentation standards for the SkillLite project.

## Language Standards

### Code Comments

**Primary Language: English**

All code comments, docstrings, and inline comments should be written in English for:
- International collaboration
- Consistency across the codebase
- Better tooling support (linters, documentation generators)

#### Examples

✅ **Good**:
```python
def calculate_timeout(base_timeout: int, multiplier: float) -> int:
    """
    Calculate the effective timeout with multiplier.
    
    Args:
        base_timeout: Base timeout in seconds
        multiplier: Timeout multiplier factor
        
    Returns:
        Calculated timeout in seconds
    """
    return int(base_timeout * multiplier)
```

❌ **Avoid**:
```python
def calculate_timeout(base_timeout: int, multiplier: float) -> int:
    """
    计算超时时间。
    
    参数:
        base_timeout: 基础超时时间（秒）
        multiplier: 超时倍数
        
    返回:
        计算后的超时时间（秒）
    """
    return int(base_timeout * multiplier)
```

### Exception: User-Facing Content

Chinese can be used in:
- User-facing messages in Chinese localization
- README_CN.md and other Chinese documentation
- Example code demonstrating Chinese language support

```python
# OK: Chinese in user-facing content
SUPPORTED_PROVIDERS = {
    "qwen": "Qwen (通义千问)",  # Chinese name for Chinese users
    "moonshot": "Moonshot (月之暗面)",
}
```

## README Synchronization

### Structure Requirements

Both `README.md` and `README_CN.md` must maintain:
- Same number of sections (headings)
- Same code examples
- Same tables
- Same external links

### Sync Workflow

1. **Primary Document**: Write/update `README.md` first
2. **Translation**: Update `README_CN.md` to match
3. **Verification**: Run sync check script

```bash
python skilllite-sdk/scripts/check_docs_sync.py
```

### When Updating Documentation

1. Update the primary language version first
2. Update the translated version
3. Run the sync check script
4. Commit both files together

## File Naming Conventions

| Type | Convention | Example |
|------|------------|---------|
| English docs | `*.md` | `README.md`, `CONTRIBUTING.md` |
| Chinese docs | `*_CN.md` | `README_CN.md` |
| Code files | English names | `executor.py`, `manager.py` |

## Docstring Format

Use Google-style docstrings:

```python
def function_name(param1: str, param2: int) -> bool:
    """
    Short description of the function.
    
    Longer description if needed. This can span multiple lines
    and provide more context about the function's behavior.
    
    Args:
        param1: Description of param1
        param2: Description of param2
        
    Returns:
        Description of return value
        
    Raises:
        ValueError: When param1 is empty
        TypeError: When param2 is not an integer
        
    Example:
        >>> function_name("test", 42)
        True
    """
    pass
```

## Automation

### Pre-commit Hook (Optional)

Add to `.git/hooks/pre-commit`:

```bash
#!/bin/bash
python skilllite-sdk/scripts/check_docs_sync.py
if [ $? -ne 0 ]; then
    echo "Documentation sync check failed!"
    exit 1
fi
```

### CI Integration

Add to your CI workflow:

```yaml
- name: Check documentation sync
  run: python skilllite-sdk/scripts/check_docs_sync.py
```

## Summary

| Aspect | Standard |
|--------|----------|
| Code comments | English |
| Docstrings | English (Google style) |
| README.md | English |
| README_CN.md | Chinese (synced with README.md) |
| Variable names | English |
| Error messages | English (can have i18n) |
