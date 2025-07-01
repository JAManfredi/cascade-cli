#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${CYAN}=================================================${NC}"
    echo -e "${CYAN} $1"
    echo -e "${CYAN}=================================================${NC}"
}

print_step() {
    echo -e "${BLUE}==>${NC} $1"
}

print_success() {
    echo -e "${GREEN}✅${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠️${NC} $1"
}

print_error() {
    echo -e "${RED}❌${NC} $1"
}

# Function to run a command and check its result
run_check() {
    local step_name="$1"
    local command="$2"
    local allow_failure="${3:-false}"
    
    print_step "$step_name"
    if eval "$command"; then
        print_success "$step_name passed"
        return 0
    else
        if [ "$allow_failure" = "true" ]; then
            print_warning "$step_name failed (allowed)"
            return 0
        else
            print_error "$step_name failed"
            return 1
        fi
    fi
}

# Simulate CI environment variables
export CARGO_TERM_COLOR=always
export CI=true
export GITHUB_ACTIONS=true
export RUST_LOG=info

print_header "CI Environment Simulation"
echo "This script simulates the GitHub Actions CI environment as closely as possible"
echo "to catch environment-specific issues before they reach CI."
echo

# Detect OS for platform-specific behavior
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    OS_TYPE="ubuntu-latest"
    print_step "Detected Linux environment (simulating ubuntu-latest)"
elif [[ "$OSTYPE" == "darwin"* ]]; then
    OS_TYPE="macos-latest"
    print_step "Detected macOS environment (simulating macos-latest)"
elif [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
    OS_TYPE="windows-latest"
    print_step "Detected Windows environment (simulating windows-latest)"
else
    OS_TYPE="unknown"
    print_warning "Unknown OS type: $OSTYPE"
fi

# Configure Git like CI does
print_step "Configuring Git (CI-style)"
git config --global user.name "CI Simulation" 2>/dev/null || true
git config --global user.email "ci@local.dev" 2>/dev/null || true
git config --global init.defaultBranch main 2>/dev/null || true

# Clean up any previous build artifacts
print_step "Cleaning previous build artifacts"
cargo clean

FAILED_CHECKS=()

# 1. Check formatting (exact CI command)
if ! run_check "Code Formatting" "cargo fmt --all -- --check"; then
    FAILED_CHECKS+=("formatting")
fi

# 2. Run Clippy (exact CI command)
if ! run_check "Clippy Linting" "cargo clippy --all-targets --all-features -- -D warnings"; then
    FAILED_CHECKS+=("clippy")
fi

# 3. Build (exact CI command)
if ! run_check "Build Check" "cargo build --verbose"; then
    FAILED_CHECKS+=("build")
fi

# 4. Unit tests (exact CI command)
if ! run_check "Unit Tests" "cargo test --lib --verbose"; then
    FAILED_CHECKS+=("unit-tests")
fi

# 5. Build release binary (exact CI command)
if ! run_check "Release Binary Build" "cargo build --release"; then
    FAILED_CHECKS+=("release-build")
fi

# 6. Integration tests with CI-like conditions
print_step "Integration Tests (CI Environment Simulation)"

# Set up CI-like environment for integration tests
export RUST_BACKTRACE=1
export TEST_TIMEOUT=300  # 5 minutes like CI
export INTEGRATION_TEST_CONCURRENCY=1  # Reduce concurrency like CI

# Run integration tests with timeout and retry logic
if timeout 900 cargo test --test '*' --verbose; then
    print_success "Integration tests passed"
else
    print_error "Integration tests failed or timed out"
    FAILED_CHECKS+=("integration-tests")
fi

# 7. CLI binary test (exact CI command)
if ! run_check "CLI Binary Test" "./target/release/ca --help"; then
    FAILED_CHECKS+=("cli-binary")
fi

# 8. Force push test (like CI does on Unix)
if [[ "$OS_TYPE" != "windows-latest" ]]; then
    if ! run_check "Force Push Test" "cargo test test_force_push --verbose" "true"; then
        print_warning "Force push test failed (non-critical)"
    fi
fi

# 9. Documentation checks (exact CI commands)
if ! run_check "Documentation Check" "cargo doc --no-deps --document-private-items"; then
    FAILED_CHECKS+=("docs")
fi

if ! run_check "Documentation Tests" "cargo test --doc"; then
    FAILED_CHECKS+=("doc-tests")
fi

# 10. MSRV check simulation
print_step "MSRV Check Simulation"
if command -v rustup &> /dev/null; then
    CURRENT_TOOLCHAIN=$(rustup show active-toolchain | cut -d' ' -f1)
    if rustup toolchain list | grep -q "1.82.0"; then
        rustup override set 1.82.0
        if cargo check; then
            print_success "MSRV check passed"
        else
            print_error "MSRV check failed"
            FAILED_CHECKS+=("msrv")
        fi
        rustup override set "$CURRENT_TOOLCHAIN"
    else
        print_warning "Rust 1.82.0 not installed, skipping MSRV check"
    fi
else
    print_warning "rustup not available, skipping MSRV check"
fi

# 11. Security audit (if available)
print_step "Security Audit"
if command -v cargo-audit &> /dev/null; then
    if cargo audit; then
        print_success "Security audit passed"
    else
        print_warning "Security audit found issues"
        FAILED_CHECKS+=("security")
    fi
else
    print_warning "cargo-audit not installed (install with: cargo install cargo-audit)"
fi

# 12. Test isolation check
print_step "Test Isolation Check"
echo "Running integration tests twice to check for state pollution..."
if cargo test --test '*' --verbose >/dev/null 2>&1; then
    if cargo test --test '*' --verbose >/dev/null 2>&1; then
        print_success "Test isolation check passed"
    else
        print_error "Tests failed on second run - possible state pollution"
        FAILED_CHECKS+=("test-isolation")
    fi
else
    print_warning "Cannot run isolation check due to failing tests"
fi

# 13. Resource cleanup check
print_step "Resource Cleanup Check"
TEMP_DIRS_BEFORE=$(find /tmp -name "cascade-cli-*" -type d 2>/dev/null | wc -l || echo 0)
cargo test --test '*' --verbose >/dev/null 2>&1 || true
sleep 2  # Give time for cleanup
TEMP_DIRS_AFTER=$(find /tmp -name "cascade-cli-*" -type d 2>/dev/null | wc -l || echo 0)

if [ "$TEMP_DIRS_AFTER" -le "$TEMP_DIRS_BEFORE" ]; then
    print_success "Resource cleanup check passed"
else
    print_warning "Possible resource cleanup issues detected"
fi

# Summary
echo
print_header "CI Simulation Summary"
if [ ${#FAILED_CHECKS[@]} -eq 0 ]; then
    print_success "All checks passed! ✨ This should match CI behavior!"
    echo
    echo "🎯 Your code is likely to pass CI. The simulation closely matches the"
    echo "   GitHub Actions environment and caught no issues."
    exit 0
else
    print_error "The following checks failed:"
    for check in "${FAILED_CHECKS[@]}"; do
        echo "  • $check"
    done
    echo
    echo "💡 These failures likely indicate issues that will also occur in CI."
    echo "   Fix these issues before pushing to avoid CI failures."
    echo
    echo "🔧 Quick fixes:"
    if [[ " ${FAILED_CHECKS[@]} " =~ " formatting " ]]; then
        echo "  cargo fmt                    # Fix formatting"
    fi
    if [[ " ${FAILED_CHECKS[@]} " =~ " clippy " ]]; then
        echo "  cargo clippy --fix           # Auto-fix clippy warnings"
    fi
    if [[ " ${FAILED_CHECKS[@]} " =~ " integration-tests " ]]; then
        echo "  ./scripts/debug-integration-tests.sh  # Debug integration test issues"
    fi
    exit 1
fi 