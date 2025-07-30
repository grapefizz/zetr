#!/bin/bash

# ZETR NES Emulator Build Script

echo "Building ZETR NES Emulator..."

# Check if SDL2 is installed
if ! brew list sdl2 &>/dev/null; then
    echo "SDL2 not found. Installing..."
    brew install sdl2
fi

# Build the project
echo "Compiling..."
LIBRARY_PATH="$(brew --prefix)/lib:$LIBRARY_PATH" cargo build --release

if [ $? -eq 0 ]; then
    echo "Build successful!"
    echo ""
    echo "Usage: ./target/release/zetr <rom_file>"
    echo "Example: ./target/release/zetr donkeykong.nes"
    echo ""
    echo "Controls:"
    echo "  Arrow keys: D-pad"
    echo "  Z: A button"
    echo "  X: B button"
    echo "  A: Select"
    echo "  S: Start"
    echo "  ESC: Quit"
else
    echo "Build failed!"
    exit 1
fi
