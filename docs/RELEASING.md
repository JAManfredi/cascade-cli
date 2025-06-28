# 🚀 Release Guide

This guide is for maintainers who want to create new releases of Cascade CLI.

## 📋 **Table of Contents**

- [Release Process](#-release-process)
- [Pre-Release Checklist](#-pre-release-checklist)
- [Creating a Release](#-creating-a-release)
- [Post-Release Tasks](#-post-release-tasks)
- [Troubleshooting](#-troubleshooting)

---

## 🔄 **Release Process**

### **Automated Release Pipeline**

Cascade CLI uses GitHub Actions for fully automated cross-platform releases:

1. **Push a tag** → Triggers automated build and release
2. **GitHub Actions builds** → Cross-platform binaries (Linux, macOS, Windows)
3. **GitHub Release created** → With auto-generated release notes
4. **Binaries tested** → Installation verification on all platforms

### **Supported Platforms**

| Platform | Architecture | Binary Name |
|----------|-------------|-------------|
| **Linux** | x64 | `cc-linux-x64.tar.gz` |
| **Linux** | ARM64 | `cc-linux-arm64.tar.gz` |
| **macOS** | x64 | `cc-macos-x64.tar.gz` |
| **macOS** | ARM64 | `cc-macos-arm64.tar.gz` |
| **Windows** | x64 | `cc-windows-x64.exe.zip` |
| **Windows** | ARM64 | `cc-windows-arm64.exe.zip` |

---

## ✅ **Pre-Release Checklist**

### **Code Quality**

- [ ] **All tests pass**: `cargo test`
- [ ] **Code compiles**: `cargo build --release`
- [ ] **Linting clean**: `cargo clippy`
- [ ] **Formatting consistent**: `cargo fmt`
- [ ] **Documentation updated**: README, CHANGELOG, etc.

### **Version Management**

- [ ] **Cargo.toml version updated**: Bump version number
- [ ] **CHANGELOG.md updated**: Document new features and fixes
- [ ] **Breaking changes documented**: If any API changes
- [ ] **Migration guide created**: If needed for breaking changes

### **Documentation**

- [ ] **README.md current**: Features, installation, examples
- [ ] **User manual updated**: New commands or options
- [ ] **API documentation**: `cargo doc` generates correctly
- [ ] **Examples work**: All code examples in docs are functional

### **Testing**

```bash
# Comprehensive testing
cargo test --all-features
cargo test --no-default-features
cargo build --release

# Manual testing
./target/release/cc --version
./target/release/cc --help
./target/release/cc stack --help

# Test core workflows
./target/release/cc init
./target/release/cc stack create test-stack
./target/release/cc stack list
```

---

## 🎯 **Creating a Release**

### **Method 1: Git Tag (Recommended)**

```bash
# 1. Ensure you're on the main branch
git checkout main
git pull origin main

# 2. Create and push the tag
git tag -a v1.2.3 -m "Release v1.2.3"
git push origin v1.2.3

# 3. GitHub Actions will automatically:
#    - Build cross-platform binaries
#    - Run tests on all platforms
#    - Create GitHub release
#    - Upload binaries as assets
```

### **Method 2: Manual Trigger**

```bash
# Use GitHub's web interface:
# 1. Go to Actions tab
# 2. Select "Release" workflow
# 3. Click "Run workflow" 
# 4. Enter tag name (e.g., v1.2.3)
# 5. Click "Run workflow"
```

### **Version Numbering**

Follow [Semantic Versioning](https://semver.org/):

- **MAJOR** (v2.0.0): Breaking changes
- **MINOR** (v1.1.0): New features, backward compatible
- **PATCH** (v1.0.1): Bug fixes, backward compatible

**Examples:**
- `v1.0.0` - Initial stable release
- `v1.1.0` - Added new `cc stack merge` command
- `v1.0.1` - Fixed critical bug in rebase logic
- `v2.0.0` - Changed CLI interface (breaking)

---

## 📦 **Post-Release Tasks**

### **Verify Release**

```bash
# Check GitHub release page
open https://github.com/JAManfredi/cascade-cli/releases

# Test installation from release
curl -L https://github.com/JAManfredi/cascade-cli/releases/latest/download/cc-linux-x64.tar.gz | tar -xz
./cc --version
```

### **Update Documentation**

- [ ] **Update README.md**: Change "coming soon" to actual download links
- [ ] **Update INSTALLATION.md**: Verify installation instructions work
- [ ] **Update docs/UPCOMING.md**: Move completed features to README
- [ ] **Social media**: Announce release on relevant platforms

### **Monitor Release**

- [ ] **Check GitHub Actions**: Ensure all workflows completed successfully
- [ ] **Verify binaries**: Download and test each platform binary
- [ ] **Monitor issues**: Watch for installation or functionality issues
- [ ] **Check metrics**: Monitor download statistics

---

## 🔧 **Troubleshooting**

### **Common Issues**

**Build Failures:**

```bash
# Check specific target
cargo build --release --target x86_64-unknown-linux-gnu

# Cross-compilation issues
rustup target add aarch64-unknown-linux-gnu
```

**Test Failures:**

```bash
# Run tests with verbose output
cargo test -- --nocapture

# Test specific module
cargo test stack::tests
```

**Release Workflow Failures:**

```bash
# Check GitHub Actions logs
# Common issues:
# - Cargo.toml version not updated
# - Missing dependencies for cross-compilation
# - Test failures on specific platforms
```

### **Manual Release Recovery**

If automated release fails, you can create a manual release:

```bash
# 1. Build all targets locally
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target x86_64-apple-darwin
cargo build --release --target x86_64-pc-windows-msvc

# 2. Create packages
cd target/x86_64-unknown-linux-gnu/release
tar czf cc-linux-x64.tar.gz cc

# 3. Upload to GitHub Release manually
```

### **Rollback Process**

If a release has critical issues:

```bash
# 1. Delete the GitHub release
gh release delete v1.2.3

# 2. Delete the tag
git tag -d v1.2.3
git push origin :refs/tags/v1.2.3

# 3. Fix issues and create new release
```

---

## 🛠️ **Release Workflow Details**

### **GitHub Actions Workflow**

The release workflow (`.github/workflows/release.yml`) includes:

1. **Multi-platform builds** - Linux, macOS, Windows (x64 + ARM64)
2. **Cross-compilation** - Uses proper toolchains for each target
3. **Testing** - Runs full test suite on native platforms
4. **Packaging** - Creates compressed archives for each platform
5. **Release creation** - Auto-generates release notes and uploads binaries
6. **Installation testing** - Downloads and tests each binary

### **Release Notes Generation**

The workflow automatically generates release notes with:

- **Changelog** - Commits since last release
- **Installation instructions** - Platform-specific download commands
- **Documentation links** - User manual, onboarding guide
- **Feature highlights** - Key capabilities and improvements

### **Binary Naming Convention**

- **Format**: `cc-{platform}-{arch}.{extension}`
- **Examples**: 
  - `cc-linux-x64.tar.gz`
  - `cc-macos-arm64.tar.gz`
  - `cc-windows-x64.exe.zip`

---

## 📈 **Release Metrics**

### **Success Indicators**

- ✅ All platform builds complete successfully
- ✅ All tests pass on all platforms
- ✅ Binaries are under 50MB compressed
- ✅ Installation works on all platforms
- ✅ No critical issues reported within 24 hours

### **Monitoring**

- **GitHub Release downloads** - Track adoption
- **Issue reports** - Monitor for installation/functionality problems
- **Performance metrics** - Binary size, startup time
- **User feedback** - Community response and suggestions

---

## 🤝 **Contributing to Releases**

### **For Maintainers**

- **Release authority**: Only designated maintainers should create releases
- **Testing responsibility**: Thoroughly test before releasing
- **Communication**: Announce planned releases in advance
- **Documentation**: Keep this guide updated with process changes

### **For Contributors**

- **Pull requests**: Ensure PRs are tested and documented
- **Version bumps**: Don't bump versions in PRs (maintainers handle this)
- **Breaking changes**: Clearly document any breaking changes
- **Testing**: Include tests for new features

---

*This release guide is maintained by the Cascade CLI team. Last updated: 6/28/25 