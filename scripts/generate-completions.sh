#!/bin/bash
# Generate shell completion files for releases

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"
COMPLETIONS_DIR="$PROJECT_ROOT/completions"

echo "🔨 Building cascade-cli..."
cd "$PROJECT_ROOT"
cargo build --release

echo "📁 Creating completions directory..."
mkdir -p "$COMPLETIONS_DIR"

echo "🐚 Generating shell completions..."

# Generate bash completion
echo "  → bash"
./target/release/cc completions generate bash > "$COMPLETIONS_DIR/cc.bash"

# Generate zsh completion
echo "  → zsh"
./target/release/cc completions generate zsh > "$COMPLETIONS_DIR/_cc"

# Generate fish completion
echo "  → fish"
./target/release/cc completions generate fish > "$COMPLETIONS_DIR/cc.fish"

# Generate PowerShell completion (for Windows users)
echo "  → powershell"
./target/release/cc completions generate powershell > "$COMPLETIONS_DIR/cc.ps1"

echo "✅ Completions generated in $COMPLETIONS_DIR"
echo ""
echo "Files created:"
ls -la "$COMPLETIONS_DIR"