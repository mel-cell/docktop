#!/bin/bash

# DockTop Installation Script

set -e

echo "Installing DockTop..."

# Install Binary
INSTALL_DIR="/usr/local/bin"
BINARY_NAME="docktop"
REPO="mel-cell/docktop"

install_binary() {
    local src=$1
    echo "Installing binary to $INSTALL_DIR/$BINARY_NAME..."
    if [ -w "$INSTALL_DIR" ]; then
        cp "$src" "$INSTALL_DIR/$BINARY_NAME"
    else
        echo "Sudo permissions required to install to $INSTALL_DIR"
        sudo cp "$src" "$INSTALL_DIR/$BINARY_NAME"
    fi
    sudo chmod +x "$INSTALL_DIR/$BINARY_NAME"
}

# Try to download from GitHub Releases first
echo "Attempting to download latest release from GitHub..."
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

if [ "$OS" == "linux" ] && [ "$ARCH" == "x86_64" ]; then
    DOWNLOAD_URL="https://github.com/$REPO/releases/latest/download/docktop-linux-amd64"
    if curl -L --fail "$DOWNLOAD_URL" -o /tmp/docktop_latest; then
        echo "Download successful!"
        install_binary /tmp/docktop_latest
        rm /tmp/docktop_latest
        INSTALLED=true
    else
        echo "Failed to download release. Falling back to build from source."
    fi
else
    echo "Pre-built binary not available for $OS-$ARCH. Falling back to build from source."
fi

if [ "$INSTALLED" != "true" ]; then
    # Check for Rust
    if ! command -v cargo &> /dev/null; then
        echo "Rust is not installed. Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source $HOME/.cargo/env
    else
        echo "Rust is already installed."
    fi

    # Build the project
    echo "Building DockTop (Release)..."
    cargo build --release
    install_binary target/release/docktop
fi

echo "DockTop installed successfully!"
echo "Run 'docktop' to start the application."

# Install Config
CONFIG_DIR="$HOME/.config/docktop"
if [ ! -d "$CONFIG_DIR" ]; then
    mkdir -p "$CONFIG_DIR"
fi

if [ ! -f "$CONFIG_DIR/config.toml" ]; then
    echo "Installing default configuration to $CONFIG_DIR/config.toml..."
    cp config.toml "$CONFIG_DIR/config.toml"
else
    echo "Configuration file already exists at $CONFIG_DIR/config.toml. Skipping..."
fi

# Install Themes
THEMES_DIR="$CONFIG_DIR/themes"
if [ ! -d "$THEMES_DIR" ]; then
    mkdir -p "$THEMES_DIR"
fi

echo "Installing themes to $THEMES_DIR..."
cp -r themes/* "$THEMES_DIR/"
