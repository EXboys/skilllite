# Installing Skillbox Sandbox

This guide explains how to install the `skillbox` sandbox binary for SkillLite.

> **Note**: The first release build is currently in progress. Once the GitHub Actions workflow completes, the binaries will be available for download.

## Quick Install

### Using Python SDK (Recommended)

If you have the SkillLite Python SDK installed:

```bash
# Install the sandbox binary
skilllite install

# Check installation status
skilllite status

# Show version information
skilllite version
```

### Manual Installation

#### Linux / macOS

```bash
curl -fsSL https://raw.githubusercontent.com/EXboys/skilllite/main/install.sh | bash
```

Or download manually:

```bash
# Detect your platform
PLATFORM=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

# Map architecture
if [ "$ARCH" = "x86_64" ]; then
    ARCH="x64"
elif [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
    ARCH="arm64"
fi

# Download latest release
VERSION="v0.1.0"  # Replace with latest version
wget "https://github.com/EXboys/skilllite/releases/download/${VERSION}/skillbox-${PLATFORM}-${ARCH}.tar.gz"

# Extract and install
tar -xzf "skillbox-${PLATFORM}-${ARCH}.tar.gz"
sudo mv skillbox /usr/local/bin/
chmod +x /usr/local/bin/skillbox

# Verify installation
skillbox --version
```

## Supported Platforms

| Platform | Architecture | Binary Name |
|----------|-------------|-------------|
| macOS | Intel (x86_64) | `skillbox-darwin-x64.tar.gz` |
| macOS | Apple Silicon (ARM64) | `skillbox-darwin-arm64.tar.gz` |
| Linux | x86_64 | `skillbox-linux-x64.tar.gz` |
| Linux | ARM64 | `skillbox-linux-arm64.tar.gz` |

## Python SDK Commands

### Install

```bash
# Install latest version
skilllite install

# Install specific version
skilllite install --version 0.1.0

# Force reinstall
skilllite install --force

# Quiet mode (no progress bar)
skilllite install --quiet
```

### Status

```bash
# Check if skillbox is installed
skilllite status
```

Output example:
```
SkillLite Installation Status
========================================
✓ skillbox is installed (v0.1.0)
  Location: /Users/username/.skillbox/bin/skillbox
```

### Version

```bash
# Show version information
skilllite version
```

Output example:
```
skilllite Python SDK: v0.1.0
skillbox binary (bundled): v0.1.0
skillbox binary (installed): v0.1.0
Platform: darwin-arm64
```

### Uninstall

```bash
# Remove installed binary
skilllite uninstall
```

## Installation Locations

The `skilllite install` command installs the binary to:

- **macOS/Linux**: `~/.skillbox/bin/skillbox`

The binary search order is:
1. `~/.skillbox/bin/skillbox` (installed by Python SDK)
2. System PATH
3. `~/.cargo/bin/skillbox` (if installed via cargo)
4. `/usr/local/bin/skillbox`
5. `/usr/bin/skillbox`
6. Development build locations (for contributors)

## Automatic Installation

When you use SkillLite in your Python code, it will automatically download and install the sandbox binary if not found:

```python
from skilllite import SkillManager

# This will auto-install skillbox if needed
manager = SkillManager(skills_dir=".skills")
```

To disable auto-installation:

```python
from skilllite.sandbox.skillbox import ensure_installed

# This will raise an error if not installed
binary_path = ensure_installed(auto_install=False)
```

## Troubleshooting

### Binary not found after installation

Make sure `~/.skillbox/bin` is in your PATH:

```bash
echo 'export PATH="$HOME/.skillbox/bin:$PATH"' >> ~/.bashrc  # or ~/.zshrc
source ~/.bashrc  # or ~/.zshrc
```

### Download fails

If the download fails, you can manually download from GitHub Releases:

https://github.com/EXboys/skilllite/releases

Then extract and place the binary in `~/.skillbox/bin/`.

### Platform not supported

Currently supported platforms:
- macOS (Intel and Apple Silicon)
- Linux (x86_64 and ARM64)

Windows support is planned for future releases.

## Building from Source

If you want to build the sandbox binary yourself:

```bash
# Clone the repository
git clone https://github.com/EXboys/skilllite.git
cd skilllite/skillbox

# Build with Cargo
cargo build --release

# The binary will be at: target/release/skillbox
```

## For Developers

### Creating a Release

To create a new release with pre-built binaries:

1. Update version in `skillbox/Cargo.toml`
2. Update `BINARY_VERSION` in `skilllite-sdk/skilllite/sandbox/skillbox/binary.py`
3. Commit and push changes
4. Create and push a tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

5. GitHub Actions will automatically build and publish the binaries

