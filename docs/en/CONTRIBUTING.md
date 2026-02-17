# Contributing Guide

Thank you for your interest in contributing to SkillLite!

## Ways to Contribute

- **Bug Reports**: Open an issue with detailed reproduction steps
- **Feature Requests**: Open an issue to discuss
- **Code Contributions**: Submit a pull request
- **Documentation**: Fix typos, improve docs, add examples
- **Skills**: Create and share new Skills

## Development Setup

### Prerequisites

- **Rust** (latest stable) - for sandbox executor
- **Python 3.8+** - for SDK
- **macOS or Linux** - Windows not supported

### Setup

```bash
# Clone
git clone https://github.com/EXboys/skilllite.git
cd skilllite

# Build Rust sandbox
cd skilllite
cargo build --release
cargo install --path .

# Setup Python SDK
cd ../python-sdk
pip install -e ".[dev]"

# Configure
cp .env.example .env
```

## Pull Request Process

1. Fork the repository
2. Create branch from `main`: `git checkout -b feature/your-feature`
3. Make changes with clear commits
4. Test: `cd skilllite && cargo test` and `cd python-sdk && pytest`
5. Submit PR with clear description

### Guidelines

- Keep PRs focused and reasonably sized
- Include tests for new functionality
- Ensure CI passes before requesting review

## Code Style

### Rust
- Follow standard Rust conventions
- Run `cargo fmt` and `cargo clippy`

### Python
- Follow PEP 8
- Use type hints
- Format with `black` and `isort`

## Documentation Standards

- **Code comments**: English for all code comments and docstrings
- **User docs**: Maintain both English and Chinese versions
- **README sync**: Update both `README.md` and `README_CN.md` together

## Security

For security vulnerabilities, **DO NOT** open public issues. Contact: security@skilllite.dev

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

## Project Structure

```
skillLite/
├── skilllite/          # Rust sandbox executor
├── python-sdk/     # Python SDK
├── .skills/           # Built-in Skills (examples)
├── benchmark/         # Performance benchmarks
└── docs/              # Documentation
    ├── en/            # English docs
    └── zh/            # Chinese docs
```

## Code of Conduct

We pledge to make participation harassment-free for everyone.

**Positive behaviors:**
- Demonstrating empathy and kindness
- Being respectful of differing opinions
- Giving and accepting constructive feedback

**Unacceptable behaviors:**
- Harassment, trolling, or personal attacks
- Publishing others' private information

Report issues to: security@skilllite.dev

---

*Adapted from [Contributor Covenant](https://www.contributor-covenant.org) v2.1*

