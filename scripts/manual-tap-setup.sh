#!/bin/bash
# Manual setup for the Homebrew tap since the repo already exists

set -e

echo "🍺 Setting up the existing Homebrew tap repository..."

# Get the current directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"

# Create a temporary directory in the project
TEMP_DIR="$PROJECT_ROOT/temp-tap-setup"
mkdir -p "$TEMP_DIR"
cd "$TEMP_DIR"

echo "📥 Cloning the tap repository..."
git clone https://github.com/JAManfredi/homebrew-cascade-cli.git
cd homebrew-cascade-cli

echo "📁 Creating Formula directory..."
mkdir -p Formula

echo "📋 Copying formula..."
cp "$PROJECT_ROOT/homebrew/cascade-cli.rb" Formula/cascade-cli.rb

echo "📝 Creating README..."
cat > README.md << 'EOF'
# Homebrew Cascade CLI

Homebrew tap for [Cascade CLI](https://github.com/JAManfredi/cascade-cli).

## Installation

```bash
brew tap JAManfredi/cascade-cli
brew install cascade-cli
```

## Updating

```bash
brew update
brew upgrade cascade-cli
```

## Formula Maintenance

The formula is automatically updated when new releases are published.
EOF

echo "🚀 Committing and pushing..."
git add .
git commit -m "Add Cascade CLI formula

Initial setup of Homebrew tap for Cascade CLI.
Formula supports both ARM64 and x64 macOS architectures."
git push origin main

echo ""
echo "✅ Homebrew tap repository setup complete!"
echo ""
echo "Users can now install with:"
echo "  brew tap JAManfredi/cascade-cli"
echo "  brew install cascade-cli"
echo ""
echo "🧹 Cleaning up temporary directory..."
cd "$PROJECT_ROOT"
rm -rf "$TEMP_DIR"

echo "✨ All done!"