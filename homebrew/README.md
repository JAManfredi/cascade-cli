# Homebrew Formula Setup

This directory contains the Homebrew formula for Cascade CLI, but it requires additional setup to work properly.

## Current Status

The formula is ready but **not yet functional** because Homebrew requires tap formulas to be in a separate repository.

## Required Setup

To make `brew install JAManfredi/cascade-cli/cascade-cli` work:

1. Create a new repository at `github.com/JAManfredi/homebrew-cascade-cli`
2. Move `cascade-cli.rb` to the root of that repository
3. Users can then run:
   ```bash
   brew tap JAManfredi/cascade-cli
   brew install cascade-cli
   ```

## Alternative (Simpler) Approach

Instead of creating a tap, you could:

1. Submit the formula to homebrew-core (requires meeting their acceptance criteria)
2. Then users can simply run: `brew install cascade-cli`

## Current Installation Method

Until the Homebrew tap is set up, users should install via:

```bash
# Universal installer script
curl -fsSL https://raw.githubusercontent.com/JAManfredi/cascade-cli/master/install.sh | bash

# Or manual download
curl -L https://github.com/JAManfredi/cascade-cli/releases/latest/download/cc-macos-$(uname -m | sed 's/x86_64/x64/;s/arm64/arm64/').tar.gz | tar -xz
sudo mv cc /usr/local/bin/
```

## Testing the Formula Locally

To test this formula without setting up a tap:

```bash
brew install --build-from-source ./homebrew/cascade-cli.rb
```