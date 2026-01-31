# Contributing to SkillLite

Thank you for your interest in contributing to SkillLite! This document provides guidelines and instructions for contributing.

## 🌟 Ways to Contribute

- **Bug Reports**: Found a bug? Please open an issue with detailed reproduction steps
- **Feature Requests**: Have an idea? Open an issue to discuss it
- **Code Contributions**: Submit a pull request with your improvements
- **Documentation**: Help improve our docs, fix typos, or add examples
- **Skills**: Create and share new Skills for the community

## 🚀 Getting Started

### Prerequisites

- **Rust** (latest stable) - for building the sandbox executor
- **Python 3.8+** - for the SDK
- **macOS or Linux** - Windows is not currently supported

### Development Setup

1. **Clone the repository**
   ```bash
   git clone https://github.com/chenduan/skillLite.git
   cd skillLite
   ```

2. **Build the Rust sandbox**
   ```bash
   cd skillbox
   cargo build --release
   cargo install --path .
   ```

3. **Set up Python environment**
   ```bash
   cd skilllite-sdk
   pip install -e ".[dev]"
   ```

4. **Configure environment**
   ```bash
   cp .env.example .env
   # Edit .env with your API keys
   ```

## 📝 Pull Request Process

### Before Submitting

1. **Check existing issues/PRs** - Avoid duplicates
2. **Open an issue first** - For significant changes, discuss before coding
3. **Follow code style** - Match the existing codebase style

### Submitting a PR

1. **Fork the repository** and create your branch from `main`
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes** with clear, atomic commits

3. **Test your changes**
   ```bash
   # Rust tests
   cd skillbox && cargo test
   
   # Python tests
   cd skilllite-sdk && pytest
   ```

4. **Update documentation** if needed

5. **Submit the PR** with a clear description of changes

### PR Guidelines

- Keep PRs focused and reasonably sized
- Write meaningful commit messages
- Include tests for new functionality
- Update README/docs if adding features
- Ensure CI passes before requesting review

## 🏗️ Project Structure

```
skillLite/
├── skillbox/          # Rust sandbox executor
├── skilllite-sdk/     # Python SDK
├── .skills/           # Built-in Skills (examples)
├── benchmark/         # Performance benchmarks
└── docs/              # Documentation
```

## 💻 Code Style

### Rust

- Follow standard Rust conventions
- Run `cargo fmt` before committing
- Run `cargo clippy` and address warnings

### Python

- Follow PEP 8
- Use type hints where appropriate
- Format with `black` and `isort`

## 🔒 Security

If you discover a security vulnerability, please **DO NOT** open a public issue. Instead, see [SECURITY.md](SECURITY.md) for responsible disclosure instructions.

## 📄 License

By contributing, you agree that your contributions will be licensed under the MIT License.

## 💬 Questions?

- Open a [Discussion](https://github.com/chenduan/skillLite/discussions)
- Check existing [Issues](https://github.com/chenduan/skillLite/issues)

Thank you for contributing! 🎉
