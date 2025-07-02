#!/bin/bash

# Version Bump Script for Cascade CLI
# Automates the process of bumping version across all files and creating releases

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to validate version format
validate_version() {
    local version=$1
    if [[ ! $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        print_error "Invalid version format: $version"
        print_error "Expected format: X.Y.Z (e.g., 1.2.3)"
        exit 1
    fi
}

# Function to get current version from Cargo.toml
get_current_version() {
    grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/'
}

# Function to update Cargo.toml
update_cargo_toml() {
    local new_version=$1
    print_status "Updating Cargo.toml from $current_version to $new_version"
    
    # Use sed to update the first version line (which should be the package version)
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS version
        sed -i '' "s/^version = \".*\"/version = \"$new_version\"/" Cargo.toml
    else
        # Linux version
        sed -i "s/^version = \".*\"/version = \"$new_version\"/" Cargo.toml
    fi
    
    print_success "Updated Cargo.toml"
}

# Note: Homebrew formula is now managed in separate tap repository
# and will be updated automatically by GitHub workflow on release

# Function to update Cargo.lock
update_cargo_lock() {
    print_status "Updating Cargo.lock"
    cargo check --quiet
    print_success "Updated Cargo.lock"
}

# Function to search for other version references
check_other_references() {
    local current_version=$1
    local new_version=$2
    
    print_status "Searching for other version references..."
    
    # Search for version references in documentation and other files
    local files_with_version=$(grep -r "$current_version" --include="*.md" --include="*.toml" --include="*.rs" --include="*.sh" . || true)
    
    if [[ -n "$files_with_version" ]]; then
        print_warning "Found potential version references that may need manual review:"
        echo "$files_with_version" | grep -v "Cargo.lock" | grep -v "target/" | head -10
        echo ""
        print_warning "Please review these files manually if they contain version references"
        echo ""
    fi
}

# Function to create git commit and tag
create_git_commit_and_tag() {
    local new_version=$1
    local current_version=$2
    
    print_status "Creating git commit and tag"
    
    # Check if there are changes to commit
    if [[ -z $(git status --porcelain) ]]; then
        print_warning "No changes to commit"
        return
    fi
    
    # Add changed files
    git add Cargo.toml Cargo.lock
    
    # Create commit
    local commit_message="chore: Bump version from $current_version to $new_version

- Updated Cargo.toml package version
- Refreshed Cargo.lock dependencies

Ready for v$new_version release. Homebrew formula will be updated automatically by GitHub workflow."
    
    git commit -m "$commit_message"
    print_success "Created commit for version $new_version"
    
    # Create annotated tag
    local tag_message="Release v$new_version

Version $new_version includes all cross-platform compatibility fixes
and improvements from the previous development cycle.

This release has been tested across Ubuntu, Windows, and macOS
platforms with comprehensive CI validation."
    
    git tag -a "v$new_version" -m "$tag_message"
    print_success "Created tag v$new_version"
}

# Function to show git status and next steps
show_next_steps() {
    local new_version=$1
    
    echo ""
    print_success "Version bump to $new_version completed successfully!"
    echo ""
    print_status "Next steps:"
    echo "  1. Review the changes: git show HEAD"
    echo "  2. Push the changes: git push origin master"  
    echo "  3. Push the tag: git push origin v$new_version"
    echo "  4. Create GitHub release from the tag"
    echo "  5. Build and upload release binaries"
    echo "  6. Homebrew tap will be updated automatically by GitHub workflow"
    echo ""
    print_status "Current git status:"
    git log --oneline -n 3
}

# Main function
main() {
    echo "🚀 Cascade CLI Version Bump Script"
    echo "=================================="
    echo ""
    
    # Check if we're in the right directory
    if [[ ! -f "Cargo.toml" ]] || [[ ! -d "src" ]]; then
        print_error "Must be run from the project root directory"
        print_error "Make sure Cargo.toml and src/ directory exist"
        exit 1
    fi
    
    # Check for clean git status
    if [[ -n $(git status --porcelain) ]]; then
        print_error "Working directory is not clean. Please commit or stash changes first."
        git status --short
        exit 1
    fi
    
    # Get new version from command line argument
    if [[ $# -eq 0 ]]; then
        echo "Usage: $0 <new-version>"
        echo "Example: $0 1.2.3"
        echo ""
        echo "Current version: $(get_current_version)"
        exit 1
    fi
    
    local new_version=$1
    validate_version "$new_version"
    
    local current_version=$(get_current_version)
    
    echo "Current version: $current_version"
    echo "New version: $new_version"
    echo ""
    
    # Confirm with user
    read -p "Proceed with version bump? (y/N): " -n 1 -r
    echo ""
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        print_status "Version bump cancelled"
        exit 0
    fi
    
    echo ""
    print_status "Starting version bump process..."
    echo ""
    
    # Perform updates
    update_cargo_toml "$new_version"
    update_cargo_lock
    check_other_references "$current_version" "$new_version"
    create_git_commit_and_tag "$new_version" "$current_version"
    show_next_steps "$new_version"
}

# Run the main function
main "$@" 