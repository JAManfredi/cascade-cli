# Homebrew Formula for Cascade CLI

This directory contains the Homebrew formula for Cascade CLI.

## Installation Methods

### Method 1: Direct URL Installation (Recommended)

No tap required! Install directly from the formula URL:

```bash
brew install https://raw.githubusercontent.com/JAManfredi/cascade-cli/master/homebrew/cascade-cli.rb
```

To upgrade:
```bash
brew upgrade https://raw.githubusercontent.com/JAManfredi/cascade-cli/master/homebrew/cascade-cli.rb
```

### Method 2: Local Installation

If you've cloned the repository:
```bash
brew install ./homebrew/cascade-cli.rb
```

### Method 3: Create a Tap (Optional)

If you prefer the traditional tap approach:

1. Create a new repository at `github.com/JAManfredi/homebrew-cascade-cli`
2. Move `cascade-cli.rb` to the root of that repository
3. Users can then run:
   ```bash
   brew tap JAManfredi/cascade-cli
   brew install cascade-cli
   ```

## Formula Details

The formula:
- Automatically detects ARM64 vs x64 architecture
- Installs shell completions for bash, zsh, and fish
- Includes post-install instructions
- Has basic tests to verify installation

## Updating the Formula

When releasing a new version:
1. Update the `version` field
2. Update the `url` fields to point to the new release
3. Update the `sha256` checksums for both architectures

To calculate new checksums:
```bash
# For ARM64
shasum -a 256 cc-macos-arm64.tar.gz

# For x64
shasum -a 256 cc-macos-x64.tar.gz
```