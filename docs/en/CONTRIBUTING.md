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
- **Python 3.10+** - for SDK
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
4. Test: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`; for Python SDK artifact tests also `cargo build -p skilllite --bin skilllite` from the repo root (or set `SKILLLITE_ARTIFACT_HTTP_SERVE` to your `skilllite` executable), then `cd python-sdk && pytest`
5. Submit PR with clear description

### Guidelines

- Keep PRs focused and reasonably sized
- Include tests for new functionality
- Ensure CI passes before requesting review

## Code Style

### Rust
- Follow standard Rust conventions
- Run `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`
- Run `cargo deny check bans` from the repo root before submitting (install: `cargo install cargo-deny --locked --version 0.18.6`, or match `.github/workflows/ci.yml`). This enforces crate layering in `deny.toml`.
- PR CI runs full Rust/Python checks on Ubuntu and lightweight `cargo check` smoke on macOS and Windows.

### Python
- Follow PEP 8
- Use type hints
- Format with `black` and `isort`

## Documentation Standards

- **Code comments**: English for all code comments and docstrings
- **User docs**: Maintain both English and Chinese versions
- **README sync**: Update both `README.md` and `docs/zh/README.md` together

## Security

For security vulnerabilities, **DO NOT** open public issues. Contact: security@skilllite.ai

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

## Project Structure

```
skillLite/
├── skilllite/          # Rust sandbox executor
├── python-sdk/         # Python SDK
├── .skills/            # Built-in Skills (examples)
├── benchmark/          # Performance benchmarks
├── test/               # LangChain integration tests (run_tests.py, .gitignore)
├── tests/              # Pytest unit tests (test_core, test_mcp, test_langchain, etc.)
└── docs/               # Documentation
    ├── en/             # English docs
    └── zh/             # Chinese docs
```

### Test Directories

| Directory | Purpose | Run Command |
|-----------|---------|-------------|
| `test/` | LangChain/SkillLite integration tests, requires skills | `cd test && python run_tests.py` |
| `tests/` | Pytest unit tests for core/SDK | `cd python-sdk && pytest` or `pytest tests/` |

## Code of Conduct

We pledge to make participation harassment-free for everyone.

**Positive behaviors:**
- Demonstrating empathy and kindness
- Being respectful of differing opinions
- Giving and accepting constructive feedback

**Unacceptable behaviors:**
- Harassment, trolling, or personal attacks
- Publishing others' private information

Report issues to: security@skilllite.ai

---

*Adapted from [Contributor Covenant](https://www.contributor-covenant.org) v2.1*

