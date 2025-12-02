#!/bin/bash

# DockTop Installation Script

set -e

echo "Installing DockTop..."

# Check for Rust
if ! command -v cargo &> /dev/null; then
    echo "Rust is not installed. Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
else
    echo "Rust is already installed."
fi

# Check for Docker
if ! command -v docker &> /dev/null; then
    echo "WARNING: Docker is not installed. DockTop requires Docker to function correctly."
    echo "Please install Docker manually for your distribution."
else
    echo "Docker is installed."
fi

# Check and Install Nerd Fonts
FONT_DIR="$HOME/.local/share/fonts"
FONT_NAME="JetBrainsMonoNerdFont"

if [ ! -d "$FONT_DIR" ]; then
    mkdir -p "$FONT_DIR"
fi

if ! ls "$FONT_DIR" | grep -q "JetBrainsMono"; then
    echo "Installing Nerd Fonts (JetBrainsMono) for icons..."
    # Download font
    mkdir -p /tmp/nerdfonts
    curl -fLo "/tmp/nerdfonts/JetBrainsMono.zip" https://github.com/ryanoasis/nerd-fonts/releases/download/v3.1.1/JetBrainsMono.zip
    
    # Unzip and install
    unzip -o "/tmp/nerdfonts/JetBrainsMono.zip" -d "$FONT_DIR"
    
    # Clean up
    rm -rf /tmp/nerdfonts
    
    # Update font cache
    if command -v fc-cache &> /dev/null; then
        echo "Updating font cache..."
        fc-cache -f -v > /dev/null
    fi
    
    echo "Nerd Fonts installed! Please configure your terminal to use 'JetBrainsMono Nerd Font'."
else
    echo "Nerd Fonts seems to be installed."
fi

# Build the project
echo "Building DockTop (Release)..."
cargo build --release

# Install binary
echo "Installing binary to /usr/local/bin/docktop..."
if [ -w /usr/local/bin ]; then
    cp target/release/docktop /usr/local/bin/docktop
else
    echo "Sudo permissions required to install to /usr/local/bin"
    sudo cp target/release/docktop /usr/local/bin/docktop
fi

echo "DockTop installed successfully!"
echo "Run 'docktop' to start the application."
