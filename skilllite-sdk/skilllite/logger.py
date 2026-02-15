"""
Unified logging system for SkillLite.

This module provides a centralized logging configuration and utilities
for consistent logging across the entire SkillLite SDK.
"""

import logging
import re
import sys
from typing import Optional

# ---------------------------------------------------------------------------
# ANSI style codes for terminal output
# ---------------------------------------------------------------------------
ANSI_DIM = "\033[2m"
ANSI_BOLD = "\033[1m"
ANSI_RESET = "\033[0m"
ANSI_GREEN = "\033[32m"
ANSI_YELLOW = "\033[33m"
ANSI_CYAN = "\033[36m"

_ANSI_ESCAPE_RE = re.compile(r'\033\[[0-9;]*m')


def strip_ansi(text: str) -> str:
    """Remove ANSI escape codes from text."""
    return _ANSI_ESCAPE_RE.sub('', text)


def step_header(step: int, total: int) -> str:
    """Format a visual step separator/header line."""
    label = f" Step {step}/{total} "
    width = 50
    side = max(1, (width - len(label)) // 2)
    return "─" * side + label + "─" * side

# Default logger name
DEFAULT_LOGGER_NAME = "skilllite"

# Logger instance cache
_logger_cache: Optional[logging.Logger] = None


def get_logger(name: Optional[str] = None, verbose: bool = True) -> logging.Logger:
    """
    Get or create a logger instance.
    
    Args:
        name: Logger name (default: "skilllite")
        verbose: Whether to enable verbose logging (default: True)
        
    Returns:
        Configured logger instance
    """
    global _logger_cache
    
    logger_name = name or DEFAULT_LOGGER_NAME
    
    # Return cached logger if exists and name matches
    if _logger_cache is not None and _logger_cache.name == logger_name:
        return _logger_cache
    
    # Create new logger
    logger = logging.getLogger(logger_name)
    
    # Only configure if not already configured (avoid duplicate handlers)
    if not logger.handlers:
        # Set log level based on verbose flag
        logger.setLevel(logging.DEBUG if verbose else logging.WARNING)
        
        # Create console handler with formatted output
        handler = logging.StreamHandler(sys.stdout)
        handler.setLevel(logging.DEBUG if verbose else logging.WARNING)
        
        # Create formatter
        # Use simple format for better readability (no timestamp for console)
        formatter = logging.Formatter(
            '%(message)s',
            datefmt=None
        )
        handler.setFormatter(formatter)
        
        # Add handler to logger
        logger.addHandler(handler)
        
        # Prevent propagation to root logger
        logger.propagate = False
    
    # Update cache
    _logger_cache = logger
    
    return logger


def setup_logging(
    level: int = logging.INFO,
    format_string: Optional[str] = None,
    verbose: bool = True
) -> None:
    """
    Configure root logging for SkillLite.
    
    Args:
        level: Logging level (default: INFO)
        format_string: Custom format string (default: simple format)
        verbose: Whether to enable verbose logging (default: True)
    """
    if format_string is None:
        format_string = '%(message)s'
    
    # Configure root logger
    logging.basicConfig(
        level=level if verbose else logging.WARNING,
        format=format_string,
        handlers=[logging.StreamHandler(sys.stdout)]
    )


class LoggerMixin:
    """
    Mixin class to add logging capabilities to any class.
    
    Usage:
        class MyClass(LoggerMixin):
            def __init__(self, verbose=True):
                super().__init__(verbose=verbose)
                
            def do_something(self):
                self.logger.info("Doing something...")
    """
    
    def __init__(self, verbose: bool = True, logger_name: Optional[str] = None):
        """
        Initialize logger mixin.
        
        Args:
            verbose: Whether to enable verbose logging
            logger_name: Optional custom logger name
        """
        self._verbose = verbose
        self._logger_name = logger_name or self.__class__.__module__
        self._logger: Optional[logging.Logger] = None
    
    @property
    def logger(self) -> logging.Logger:
        """Get logger instance (lazy initialization)."""
        if self._logger is None:
            self._logger = get_logger(self._logger_name, verbose=self._verbose)
        return self._logger
    
    def _log(self, message: str, level: int = logging.INFO) -> None:
        """
        Log a message (backward compatibility with old _log method).
        
        Args:
            message: Message to log
            level: Logging level (default: INFO)
        """
        if self._verbose:
            self.logger.log(level, message)
