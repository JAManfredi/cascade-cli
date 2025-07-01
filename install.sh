#!/bin/bash
set -e

# Cascade CLI Installation Script
# This script detects your platform and downloads the appropriate binary

REPO="JAManfredi/cascade-cli"
BINARY_NAME="ca"
INSTALL_DIR="/usr/local/bin"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_info() {
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

# Detect platform
detect_platform() {
    local os="$(uname -s)"
    local arch="$(uname -m)"
    
    case $os in
        Linux*)
            case $arch in
                x86_64) echo "ca-linux-x64.tar.gz" ;;
                aarch64|arm64) echo "ca-linux-arm64.tar.gz" ;;
                *) print_error "Unsupported architecture: $arch"; exit 1 ;;
            esac
            ;;
        Darwin*)
            case $arch in
                x86_64) echo "ca-macos-x64.tar.gz" ;;
                arm64) echo "ca-macos-arm64.tar.gz" ;;
                *) print_error "Unsupported architecture: $arch"; exit 1 ;;
            esac
            ;;
        *)
            print_error "Unsupported operating system: $os"
            exit 1
            ;;
    esac
}

# Get latest release version
get_latest_version() {
    local latest_url="https://api.github.com/repos/$REPO/releases/latest"
    
    if command -v curl >/dev/null 2>&1; then
        curl -s "$latest_url" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
    elif command -v wget >/dev/null 2>&1; then
        wget -qO- "$latest_url" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
    else
        print_error "Neither curl nor wget is available. Please install one of them."
        exit 1
    fi
}

# Download and install
install_cascade() {
    local platform="$(detect_platform)"
    local version="${VERSION:-$(get_latest_version)}"
    
    if [ -z "$version" ]; then
        print_error "Could not determine latest version"
        exit 1
    fi
    
    print_info "Installing Cascade CLI $version for platform: $platform"
    
    local download_url="https://github.com/$REPO/releases/download/$version/$platform"
    local temp_dir="$(mktemp -d)"
    local archive_path="$temp_dir/$platform"
    
    print_info "Downloading from: $download_url"
    
    # Download
    if command -v curl >/dev/null 2>&1; then
        curl -L -o "$archive_path" "$download_url"
    elif command -v wget >/dev/null 2>&1; then
        wget -O "$archive_path" "$download_url"
    else
        print_error "Neither curl nor wget is available"
        exit 1
    fi
    
    # Extract
    print_info "Extracting archive..."
    cd "$temp_dir"
    tar -xzf "$archive_path"
    
    # Install
    if [ -w "$INSTALL_DIR" ]; then
        print_info "Installing to $INSTALL_DIR..."
        mv "$BINARY_NAME" "$INSTALL_DIR/"
    else
        print_info "Installing to $INSTALL_DIR (requires sudo)..."
        sudo mv "$BINARY_NAME" "$INSTALL_DIR/"
    fi
    
    # Make executable
    chmod +x "$INSTALL_DIR/$BINARY_NAME"
    
    # Cleanup
    rm -rf "$temp_dir"
    
    print_success "Cascade CLI installed successfully!"
    print_info "Run 'ca --help' to get started"
    
    # Verify installation
    if command -v ca >/dev/null 2>&1; then
        print_success "Verification: $(ca --version)"
    else
        print_warning "Binary installed but not in PATH. You may need to restart your shell."
    fi
}

# Check dependencies
check_dependencies() {
    local deps=("tar")
    local missing=()
    
    for dep in "${deps[@]}"; do
        if ! command -v "$dep" >/dev/null 2>&1; then
            missing+=("$dep")
        fi
    done
    
    if [ ${#missing[@]} -ne 0 ]; then
        print_error "Missing dependencies: ${missing[*]}"
        print_info "Please install the missing dependencies and try again"
        exit 1
    fi
}

# Main installation
main() {
    echo "🌊 Cascade CLI Installer"
    echo "========================"
    
    check_dependencies
    install_cascade
    
    echo ""
    echo "🎉 Installation complete!"
    echo ""
    echo "Next steps:"
    echo "  1. Run 'ca setup' in your Git repository"
    echo "  2. Check out the documentation: https://github.com/$REPO"
    echo "  3. Join our community for support and updates"
}

# Handle command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --version)
            VERSION="$2"
            shift 2
            ;;
        --install-dir)
            INSTALL_DIR="$2"
            shift 2
            ;;
        --help)
            echo "Cascade CLI Installer"
            echo ""
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --version VERSION      Install specific version (default: latest)"
            echo "  --install-dir DIR      Install to specific directory (default: /usr/local/bin)"
            echo "  --help                 Show this help message"
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            exit 1
            ;;
    esac
done

main 