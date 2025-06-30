#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

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
    
    print_step "$step_name"
    if eval "$command"; then
        print_success "$step_name passed"
        return 0
    else
        print_error "$step_name failed"
        return 1
    fi
}

echo "🚀 Running all CI checks locally before push..."
echo "This will catch any issues before they reach GitHub!"
echo
print_warning "Note: Some environment-dependent issues (like default Git branch names)"
print_warning "may still occur in CI even if tests pass locally. Consider testing with"
print_warning "different Git configurations if CI failures don't match local results."
echo

# Configure git (in case it's not set)
git config --global user.name "$(git config user.name || echo 'Local User')" 2>/dev/null || true
git config --global user.email "$(git config user.email || echo 'user@local.dev')" 2>/dev/null || true

FAILED_CHECKS=()

# 1. Check formatting
if ! run_check "Code Formatting" "cargo fmt --all -- --check"; then
    FAILED_CHECKS+=("formatting")
    print_warning "Run 'cargo fmt' to fix formatting issues"
fi

# 2. Run Clippy (the linter that's currently failing)
if ! run_check "Clippy Linting" "cargo clippy --all-targets --all-features -- -D warnings"; then
    FAILED_CHECKS+=("clippy")
    print_warning "Fix clippy warnings or run 'cargo clippy --fix' for auto-fixes"
fi

# 3. Build check
if ! run_check "Build Check" "cargo build --verbose"; then
    FAILED_CHECKS+=("build")
fi

# 4. Unit tests
if ! run_check "Unit Tests" "cargo test --lib --verbose"; then
    FAILED_CHECKS+=("unit-tests")
fi

# 5. Integration tests (allow failures like CI does)
print_step "Integration Tests"
if cargo test --test '*' --verbose -- --test-threads=1; then
    print_success "Integration tests passed"
else
    print_warning "Integration tests failed (allowed in CI)"
fi

# 6. CLI binary test
if ! run_check "CLI Binary Test" "cargo build --release && ./target/release/csc --help > /dev/null"; then
    FAILED_CHECKS+=("cli-binary")
fi

# 7. Documentation check
if ! run_check "Documentation Check" "cargo doc --no-deps --document-private-items"; then
    FAILED_CHECKS+=("docs")
fi

# 8. Documentation tests
if ! run_check "Documentation Tests" "cargo test --doc"; then
    FAILED_CHECKS+=("doc-tests")
fi

# 9. Security audit (optional - might not have cargo-audit installed)
print_step "Security Audit"
if command -v cargo-audit &> /dev/null; then
    if cargo audit; then
        print_success "Security audit passed"
    else
        print_warning "Security audit found issues"
        FAILED_CHECKS+=("security")
    fi
else
    print_warning "cargo-audit not installed (run: cargo install cargo-audit)"
fi

# Summary
echo
echo "🎯 Summary:"
if [ ${#FAILED_CHECKS[@]} -eq 0 ]; then
    print_success "All checks passed! ✨ Safe to push to GitHub!"
    exit 0
else
    print_error "The following checks failed:"
    for check in "${FAILED_CHECKS[@]}"; do
        echo "  • $check"
    done
    echo
    echo "💡 Quick fixes:"
    if [[ " ${FAILED_CHECKS[@]} " =~ " formatting " ]]; then
        echo "  cargo fmt                    # Fix formatting"
    fi
    if [[ " ${FAILED_CHECKS[@]} " =~ " clippy " ]]; then
        echo "  cargo clippy --fix           # Auto-fix clippy warnings"
    fi
    echo
    echo "🔧 Advanced debugging (for CI-specific issues):"
    echo "  ./scripts/ci-simulation.sh           # Enhanced CI environment simulation"
    echo "  ./scripts/debug-integration-tests.sh # Debug integration test failures"
    echo "  ./scripts/docker-ci-simulation.sh    # Docker-based exact CI replica"
    echo
    print_error "Please fix these issues before pushing!"
    exit 1
fi 