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
