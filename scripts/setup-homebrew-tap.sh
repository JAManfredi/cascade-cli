#!/bin/bash
# Script to set up a Homebrew tap repository for Cascade CLI

set -e

echo "This script will help you set up a Homebrew tap for Cascade CLI"
echo "Prerequisites:"
echo "  - GitHub CLI (gh) installed and authenticated"
echo "  - Write access to JAManfredi GitHub account"
echo ""
read -p "Continue? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 1
fi

# Create the tap repository
echo "Creating homebrew-cascade-cli repository..."
gh repo create JAManfredi/homebrew-cascade-cli --public --description "Homebrew tap for Cascade CLI" || {
    echo "Repository might already exist, continuing..."
}

# Clone it
TEMP_DIR=$(mktemp -d)
cd "$TEMP_DIR"
git clone https://github.com/JAManfredi/homebrew-cascade-cli.git || {
    echo "Failed to clone repository. Make sure it exists and you have access."
    exit 1
}

cd homebrew-cascade-cli

# Create Formula directory (Homebrew convention)
mkdir -p Formula

# Copy the formula
echo "Copying formula..."
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cp "$SCRIPT_DIR/../homebrew/cascade-cli.rb" ./Formula/cascade-cli.rb

# Create README
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

# Commit and push
git add .
git commit -m "Add Cascade CLI formula"
git push origin main

echo ""
echo "✅ Homebrew tap repository created successfully!"
echo ""
echo "Users can now install with:"
echo "  brew tap JAManfredi/cascade-cli"
echo "  brew install cascade-cli"
echo ""
echo "Remember to update the formula's version and checksums when releasing new versions."