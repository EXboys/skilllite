#!/bin/bash
# SkillLite Installation Script
# Usage: curl -fsSL https://raw.githubusercontent.com/EXboys/skilllite/main/install.sh | bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
REPO="EXboys/skilllite"
BINARY_NAME="skilllite"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Detect OS and Architecture
detect_platform() {
    local os=$(uname -s | tr '[:upper:]' '[:lower:]')
    local arch=$(uname -m)
    
    case "$os" in
        linux*)
            OS="linux"
            ;;
        darwin*)
            OS="macos"
            ;;
        msys*|mingw*|cygwin*)
            OS="windows"
            ;;
        *)
            echo -e "${RED}Unsupported OS: $os${NC}"
            exit 1
            ;;
    esac
    
    case "$arch" in
        x86_64|amd64)
            ARCH="x86_64"
            ;;
        aarch64|arm64)
            ARCH="arm64"
            ;;
        *)
            echo -e "${RED}Unsupported architecture: $arch${NC}"
            exit 1
            ;;
    esac
    
    # Construct download asset name (matches GitHub release format)
    # Release assets: skilllite-linux-x64, skilllite-darwin-arm64, etc.
    case "$ARCH" in
        x86_64) ARCH_SUFFIX="x64" ;;
        arm64) ARCH_SUFFIX="arm64" ;;
        *) ARCH_SUFFIX="$ARCH" ;;
    esac
    if [ "$OS" = "macos" ]; then
        OS_SUFFIX="darwin"
    else
        OS_SUFFIX="$OS"
    fi
    if [ "$OS" = "windows" ]; then
        BINARY_FILE="${BINARY_NAME}-${OS_SUFFIX}-${ARCH_SUFFIX}.zip"
    else
        BINARY_FILE="${BINARY_NAME}-${OS_SUFFIX}-${ARCH_SUFFIX}.tar.gz"
    fi
    
    echo -e "${GREEN}Detected platform: ${OS}-${ARCH}${NC}"
}

# Get latest release version
get_latest_release() {
    echo -e "${YELLOW}Fetching latest release...${NC}"
    LATEST_RELEASE=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    
    if [ -z "$LATEST_RELEASE" ]; then
        echo -e "${RED}Failed to fetch latest release${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}Latest version: ${LATEST_RELEASE}${NC}"
}

# Download binary
download_binary() {
    local download_url="https://github.com/${REPO}/releases/download/${LATEST_RELEASE}/${BINARY_FILE}"
    local temp_file="/tmp/${BINARY_FILE}"
    
    echo -e "${YELLOW}Downloading from: ${download_url}${NC}"
    
    if command -v curl &> /dev/null; then
        curl -fsSL -o "$temp_file" "$download_url"
    elif command -v wget &> /dev/null; then
        wget -q -O "$temp_file" "$download_url"
    else
        echo -e "${RED}Neither curl nor wget found. Please install one of them.${NC}"
        exit 1
    fi
    
    if [ ! -f "$temp_file" ]; then
        echo -e "${RED}Download failed${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}Download completed${NC}"
    echo "$temp_file"
}

# Install binary
install_binary() {
    local temp_file=$1
    
    # Create install directory if it doesn't exist
    mkdir -p "$INSTALL_DIR"
    
    local binary_path="${INSTALL_DIR}/${BINARY_NAME}"
    if [[ "$temp_file" == *.tar.gz ]]; then
        tar -xzf "$temp_file" -C "$INSTALL_DIR"
        rm -f "$temp_file"
    elif [[ "$temp_file" == *.zip ]]; then
        unzip -o "$temp_file" -d "$INSTALL_DIR"
        rm -f "$temp_file"
    else
        mv "$temp_file" "$binary_path"
    fi
    chmod +x "$binary_path"
    
    echo -e "${GREEN}Installed to: ${binary_path}${NC}"
    
    # Check if install directory is in PATH
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        echo -e "${YELLOW}Warning: ${INSTALL_DIR} is not in your PATH${NC}"
        echo -e "${YELLOW}Add the following line to your ~/.bashrc or ~/.zshrc:${NC}"
        echo -e "${GREEN}export PATH=\"\$PATH:${INSTALL_DIR}\"${NC}"
    fi
}

# Main installation process
main() {
    echo -e "${GREEN}=== SkillLite Installation ===${NC}"
    
    detect_platform
    get_latest_release
    temp_file=$(download_binary)
    install_binary "$temp_file"
    
    echo -e "${GREEN}=== Installation Complete ===${NC}"
    echo -e "${GREEN}Run '${BINARY_NAME} --help' to get started${NC}"
}

main

