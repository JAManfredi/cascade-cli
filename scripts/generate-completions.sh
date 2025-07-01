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
./target/release/ca completions generate bash > "$COMPLETIONS_DIR/ca.bash"

# Generate zsh completion
echo "  → zsh"
./target/release/ca completions generate zsh > "$COMPLETIONS_DIR/_ca"

# Generate fish completion
echo "  → fish"
./target/release/ca completions generate fish > "$COMPLETIONS_DIR/ca.fish"

# Generate PowerShell completion (for Windows users)
echo "  → powershell"
./target/release/ca completions generate powershell > "$COMPLETIONS_DIR/ca.ps1"

echo "✅ Completions generated in $COMPLETIONS_DIR"
echo ""
echo "Files created:"
ls -la "$COMPLETIONS_DIR"