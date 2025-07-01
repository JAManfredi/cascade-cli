# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

### Build and Development
```bash
# Build debug version
cargo build

# Build release version
cargo build --release

# Run specific test
cargo test test_name

# Run unit tests only
cargo test --lib

# Run integration tests only
cargo test --test '*'

# Run tests with single thread (required for integration tests)
cargo test -- --test-threads=1

# Run pre-commit validation (always run before pushing)
./scripts/pre-push-check.sh
```

### Git Hook Setup
To automatically run formatting and linting checks before pushing:
```bash
# Install the pre-push Git hook (one-time setup)
cp .git/hooks/pre-push.sample .git/hooks/pre-push 2>/dev/null || true
cat > .git/hooks/pre-push << 'EOF'
#!/bin/bash
GIT_DIR=$(git rev-parse --show-toplevel)
if [ -f "$GIT_DIR/scripts/pre-push-check.sh" ]; then
    echo "Running pre-push checks..."
    "$GIT_DIR/scripts/pre-push-check.sh"
    exit $?
fi
EOF
chmod +x .git/hooks/pre-push
```

The pre-push hook will automatically run:
- `cargo fmt --all -- --check` (formatting check)
- `cargo clippy --all-targets --all-features -- -D warnings` (linting)
- All other CI checks before allowing push

If any check fails, the push will be prevented.

### Linting and Formatting
```bash
# Check formatting
cargo fmt --all -- --check

# Format code
cargo fmt

# Run clippy linting
cargo clippy --all-targets --all-features -- -D warnings

# Auto-fix clippy issues
cargo clippy --fix
```

### CI Debugging
```bash
# Simulate CI environment locally
./scripts/ci-simulation.sh

# Debug integration test failures
./scripts/debug-integration-tests.sh

# Docker-based exact CI replica
./scripts/docker-ci-simulation.sh
```

## Architecture

### High-Level Structure
This is a Rust CLI tool (`ca`) that implements stacked diffs for Bitbucket Server. The core architecture follows these principles:

**Command Architecture**: Built with `clap` using a hierarchical command structure where main commands have subcommands (e.g., `ca stacks create`, `ca push`). Shortcuts exist for common operations (`ca stack` vs `ca stacks show`).

**Module Organization**:
- `cli/` - Command-line interface and command implementations
- `stack/` - Core stack management logic with metadata tracking
- `git/` - Git operations using `git2` library (no subprocess calls)
- `bitbucket/` - Bitbucket Server API integration
- `config/` - Configuration management with validation
- `utils/` - Cross-platform utilities, especially file operations

### Core Concepts

**Stack Management**: Each "stack" represents a chain of related commits that become separate pull requests. The system tracks:
- Stack metadata in `.cascade/stacks.json`
- Individual entries with branch names and PR relationships
- Dependency relationships between stack entries

**Atomic Operations**: All file operations use atomic write patterns (write to temp file + rename) with platform-specific file locking to prevent corruption during concurrent access.

**Platform-Specific Handling**: The codebase has intentional platform differences:
- Windows: Longer timeouts, retry mechanisms for file operations
- Unix: More aggressive concurrency, standard file operations
- Cross-platform path validation and executable detection

### Integration Points

**Git Integration**: Uses `git2` library directly (not subprocess) for:
- Repository operations and branch management
- Commit analysis and rebase operations  
- Hook installation and management

**Bitbucket Integration**: Direct REST API calls to Bitbucket Server for:
- Pull request creation and management
- Repository validation and authentication
- Merge status checking and auto-landing

**File System**: Robust file handling with:
- Atomic writes with file locking
- Path traversal protection
- Configuration file corruption recovery

## Testing Strategy

### Test Organization
- **Unit tests**: In `src/` modules using `#[cfg(test)]`
- **Integration tests**: In `tests/integration/` with real CLI binary execution
- **Platform-specific tests**: Using `#[cfg(unix)]` and `#[cfg(windows)]` attributes

### Test Helpers
The `tests/integration/test_helpers.rs` module provides:
- Cross-platform binary path resolution (release vs debug)
- Timeout wrappers for CLI operations
- Parallel operation testing with concurrency limits
- Git repository setup with CI-compatible configuration

### Platform Testing
Integration tests expect different behavior on different platforms:
- Unix: Higher concurrency success rates expected
- Windows: File locking failures are normal and tested
- CI environments: Use conservative concurrency limits

### Critical Testing Notes
- Integration tests require building the binary first: `cargo build --release`
- File locking tests verify platform-specific timeout behavior
- Network failure tests use mocked Bitbucket APIs with `mockito`

## Development Practices

### File Operations
Always use the atomic file utilities in `src/utils.rs`:
- `atomic_file::write_json()` for configuration files
- `atomic_file::write_string()` for text files
- Platform-specific file locking is handled automatically

### Platform Utilities (`src/utils/platform.rs`)
Use these utilities for cross-platform compatibility:
- `executable_name(name)` - Adds `.exe` on Windows, nothing on Unix
- `path_separator()` - Returns `;` on Windows, `:` on Unix
- `is_executable(path)` - Checks if file is executable (permission-based on Unix, extension-based on Windows)
- `make_executable(path)` - Makes file executable (sets permissions on Unix, no-op on Windows)
- `shell_completion_dirs()` - Returns platform-specific shell completion directories
- `git_hook_extension()` - Returns `.bat` on Windows, empty on Unix
- `create_git_hook_content(name, command)` - Creates platform-specific Git hook scripts
- `normalize_line_endings(content)` - Normalizes all line endings to Unix format
- `secure_temp_dir()` - Creates temporary directory with proper permissions

### Error Handling
Use the custom `CascadeError` type with context:
- Configuration errors: `CascadeError::config()`
- Git operation errors: wrap `git2::Error`
- Network errors: wrap `reqwest::Error`

### Cross-Platform Code
When adding platform-specific code:
- Use the utilities in `src/utils/platform.rs` for common platform operations
- Use `#[cfg(windows)]` and `#[cfg(not(windows))]` attributes for platform-specific implementations
- Consider different timeout values for file operations (Windows needs longer timeouts)
- Test concurrent operations on both platforms with platform-specific expectations
- Use `Path`/`PathBuf` instead of string manipulation
- Always use `platform::executable_name()` for binary names
- Use `platform::path_separator()` for PATH environment variable parsing
- Use `platform::is_executable()` and `platform::make_executable()` for cross-platform executable handling

### CLI Command Implementation
Commands follow the pattern:
1. Parse arguments in `cli/mod.rs`
2. Implement logic in `cli/commands/`
3. Use async throughout for network operations
4. Provide both verbose and standard output modes

## Configuration System

The configuration is stored in `.cascade/config.json` with:
- Bitbucket Server connection details
- Git workflow preferences
- Stack management settings
- Rebase and conflict resolution options

Configuration loading includes validation and corruption recovery mechanisms.

## Installation Scripts

### Unix/Linux/macOS (`install.sh`)
- Bash script for Unix-like systems
- Detects architecture automatically
- Downloads and installs binary to `/usr/local/bin`

### Windows (`install.ps1`)
- PowerShell script for Windows systems
- Supports both x64 and x86 architectures  
- Installs to `$env:LOCALAPPDATA\cascade-cli`
- Automatically adds to user PATH
- Includes shell completion installation option

Usage: `powershell -ExecutionPolicy Bypass -File install.ps1`

## Important Notes

### Pre-Push Validation
**ALWAYS run `./scripts/pre-push-check.sh` before pushing changes.** This script runs:
- Formatting checks (`cargo fmt`)
- Linting (`cargo clippy`)
- Build verification
- Unit and integration tests
- Documentation checks

The Git hook setup (see above) will automate this, but if working without hooks, manual execution is critical to prevent CI failures.