# Manual Homebrew Tap Setup Instructions

Since the script had issues, here are the manual steps to set up your Homebrew tap:

## Step 1: Clone the empty repository

```bash
cd /tmp
git clone https://github.com/JAManfredi/homebrew-cascade-cli.git
cd homebrew-cascade-cli
```

## Step 2: Create the directory structure

```bash
mkdir -p Formula
```

## Step 3: Copy the files

The files you need are in your cascade-cli repo at `temp-tap-files/`:

```bash
# Copy the formula
cp /path/to/cascade-cli/temp-tap-files/Formula/cascade-cli.rb Formula/cascade-cli.rb

# Copy the README
cp /path/to/cascade-cli/temp-tap-files/README.md README.md
```

## Step 4: Commit and push

```bash
git add .
git commit -m "Add Cascade CLI formula

Initial setup of Homebrew tap for Cascade CLI.
Formula supports both ARM64 and x64 macOS architectures."
git push origin main
```

If you get an error about "main" vs "master", try:
```bash
git push origin master
```

## Step 5: Test the installation

```bash
brew tap JAManfredi/cascade-cli
brew install cascade-cli
csc --version
```

## Alternative: Use Git directly from cascade-cli directory

```bash
# From your cascade-cli directory
cd /tmp
git clone https://github.com/JAManfredi/homebrew-cascade-cli.git
cd homebrew-cascade-cli

# Copy files from the cascade-cli repo
mkdir -p Formula
cp /Users/jared/Documents/Development/Rust/cascade-cli/temp-tap-files/Formula/cascade-cli.rb Formula/
cp /Users/jared/Documents/Development/Rust/cascade-cli/temp-tap-files/README.md .

# Commit and push
git add .
git commit -m "Add Cascade CLI formula"
git push
```

The files are ready in your `temp-tap-files/` directory!