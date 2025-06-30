<div align="center">

![Cascade CLI Banner](./assets/banner.svg)

[![Rust](https://img.shields.io/badge/rust-1.82%2B-orange.svg)](https://rustup.rs/)
[![CI](https://github.com/JAManfredi/cascade-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/JAManfredi/cascade-cli/actions/workflows/ci.yml)
[![Beta Release](https://img.shields.io/badge/status-beta-orange.svg)](./docs/DEVELOPMENT.md)
[![Tests](https://img.shields.io/badge/tests-98%20passing-brightgreen.svg)](#testing)

</div>

Cascade CLI improves Git workflows by enabling **stacked diffs** for Bitbucket Server - a powerful technique for managing chains of related commits as separate, reviewable pull requests. Perfect for feature development, bug fixes, and complex changes that benefit from incremental review.

## 📋 Table of Contents

- [✨ Key Features](#key-features)
- [🌿 How Stacked Diffs Work: Branch Management](#how-stacked-diffs-work-branch-management)
- [🌟 Why Stacked Diffs?](#why-stacked-diffs)
- [🚀 Quick Start](#quick-start)
  - [1. Installation](#1-installation)
  - [2. Initialize Your Repository](#2-initialize-your-repository)
  - [3. Create Your First Stack](#3-create-your-first-stack)
  - [4. Experience the Magic](#4-experience-the-magic)
- [🔧 Git Hooks (Recommended)](#git-hooks-recommended)
- [⚡ Essential Commands](#essential-commands)
- [📖 Command Reference](#command-reference)
- [🤖 Smart Conflict Resolution](#smart-conflict-resolution)
- [🔧 Configuration](#configuration)
- [🎨 Advanced Features](#advanced-features)
- [📝 Documentation](#documentation)
- [🤝 Contributing](#contributing)
- [🔧 Development](#development)
- [🏗️ Architecture](#architecture)
- [🧪 Testing](#testing)
- [📜 License](#license)

## ✨ **Key Features**

### 🔄 **Stacked Diff Workflow**
- **Chain related commits** into logical, reviewable stacks
- **Independent PR reviews** while maintaining dependencies  
- **Automatic rebase management** when dependencies change
- **Smart force-push strategy** to preserve review history
- **Smart conflict resolution** - Auto-resolves 60-80% of common rebase conflicts

### 🏢 **Team Integration**

- **Bitbucket Server** native integration
- **Pull request automation** with dependency tracking  
- **Team workflow enforcement** via Git hooks
- **Progress tracking** with real-time status updates

### 🖥️ **Clean Interface**

- **Interactive TUI** for visual stack management
- **Shell completions** (bash, zsh, fish)
- **Rich visualizations** (ASCII, Mermaid, Graphviz, PlantUML)
- **Beautiful CLI** with progress bars and colored output

---

## 🌿 **How Stacked Diffs Work: Branch Management**

### **Key Insight: Each Commit = Its Own Branch + PR**

Cascade CLI **automatically creates individual branches** for each commit in your stack:

```bash
# You work normally (on main or a feature branch)
git checkout main  # or: git checkout -b my-feature-branch
csc stacks create feature-auth --base main

# Make commits as usual
git commit -m "Add user authentication endpoints"
git commit -m "Add password validation logic"  
git commit -m "Add comprehensive auth tests"

# Push to stack - Cascade CLI creates separate branches automatically:
csc stacks push  # → Creates: add-user-authentication-endpoints
csc stacks push  # → Creates: add-password-validation-logic  
csc stacks push  # → Creates: add-comprehensive-auth-tests

# Submit creates individual PRs to main:
csc stacks submit  # PR #101: add-user-authentication-endpoints → main
csc stacks submit  # PR #102: add-password-validation-logic → main (depends on #101)
csc stacks submit  # PR #103: add-comprehensive-auth-tests → main (depends on #102)
```

### **Two Workflow Options:**

#### **Option 1: Work on Main (Recommended for Solo Development)**
```bash
git checkout main
csc stacks create feature-name --base main
# Make commits directly on main (they stay local until you push to remote)
# Cascade CLI handles all branch creation and PR management
```

#### **Option 2: Work on Feature Branch (Team-Friendly)**  
```bash
git checkout -b feature-work
csc stacks create feature-name --base main
# Make commits on your feature branch
# Cascade CLI creates individual branches from your feature branch
# All PRs target main, not your feature branch
```

### **What You See vs. What Reviewers See:**

| **You (Developer)** | **Reviewers (Bitbucket)** |
|---|---|
| Work on 1 branch | See 3 separate PRs |
| 3 commits in sequence | Each PR focuses on 1 logical change |
| `git log` shows: A→B→C | PR #101: A, PR #102: B, PR #103: C |

### **The Magic: Auto-Generated Branch Names**

Cascade CLI creates meaningful branch names from your commit messages:

```bash
"Add user authentication endpoints"   → add-user-authentication-endpoints
"Fix login timeout bug"              → fix-login-timeout-bug  
"Refactor password validation!!!"    → refactor-password-validation
```

---

## 🌟 **Why Stacked Diffs?**

Traditional Git workflows often result in:
- **Large, hard-to-review PRs** 
- **Blocked development** waiting for reviews
- **Merge conflicts** from long-lived branches
- **Lost context** in massive changesets

**Stacked diffs solve this by:**
- ✅ **Small, focused PRs** that are easy to review
- ✅ **Parallel development** with dependency management  
- ✅ **Reduced conflicts** through frequent integration
- ✅ **Better code quality** via incremental feedback

---

## 🚀 **Quick Start**

### **1. Installation**

#### **Quick Install (Recommended)**

**Universal Script (Linux/macOS):**
```bash
curl -fsSL https://raw.githubusercontent.com/JAManfredi/cascade-cli/master/install.sh | bash
```

**Package Managers:**
```bash
# macOS - Homebrew (download formula first)
curl -O https://raw.githubusercontent.com/JAManfredi/cascade-cli/master/homebrew/cascade-cli.rb
brew install cascade-cli.rb
rm cascade-cli.rb

# Rust users
cargo install cascade-cli
```

#### **Manual Installation**

**Pre-built Binaries:**
```bash
# macOS (auto-detect architecture)
curl -L https://github.com/JAManfredi/cascade-cli/releases/latest/download/csc-macos-$(uname -m | sed 's/x86_64/x64/;s/arm64/arm64/').tar.gz | tar -xz
sudo mv csc /usr/local/bin/

# Linux (auto-detect architecture)  
curl -L https://github.com/JAManfredi/cascade-cli/releases/latest/download/csc-linux-$(uname -m | sed 's/x86_64/x64/;s/aarch64/arm64/').tar.gz | tar -xz
sudo mv csc /usr/local/bin/

# Windows (PowerShell)
Invoke-WebRequest -Uri "https://github.com/JAManfredi/cascade-cli/releases/latest/download/csc-windows-x64.exe.zip" -OutFile "csc.zip"
Expand-Archive -Path "csc.zip" -DestinationPath "$env:USERPROFILE\bin\"
```

**From Source:**
```bash
git clone https://github.com/JAManfredi/cascade-cli.git
cd cascade-cli
cargo build --release
cargo install --path .
```

See [Installation Guide](./docs/INSTALLATION.md) for detailed platform-specific instructions.

### **2. Initialize Your Repository**
```bash
# Navigate to your Git repository
cd my-project

# Quick setup wizard (recommended)
csc setup

# Or manual initialization
csc init --bitbucket-url https://bitbucket.company.com
```

### **3. Create Your First Stack**
```bash
# Create a new stack
csc stacks create feature-auth --base develop --description "User authentication system"

# Make multiple incremental commits (for your own tracking)
git add . && git commit -m "WIP: start authentication"
git add . && git commit -m "WIP: add login logic"
git add . && git commit -m "WIP: fix validation bugs"
git add . && git commit -m "Final: complete auth with tests"

# SQUASH + PUSH - Combine incremental commits into clean commit!
csc stacks push --squash 4  # Squashes last 4 commits into 1

# OR: Make some commits normally, then squash later ones
git commit -m "Add core authentication logic"
csc stacks push  # Push first clean commit

git commit -m "WIP: start tests"
git commit -m "WIP: more tests"  
git commit -m "Final: comprehensive test suite"

# SQUASH UNPUSHED - Only squash the last 3 commits
csc stacks push --squash 3  # Squashes and pushes as second stack entry

# BATCH SUBMIT - Submit all unsubmitted entries as separate PRs!
csc stacks submit

# Alternative options (for granular control):
csc stacks push                               # Push all unpushed commits separately
csc stacks push --since HEAD~3                # Push commits since reference
csc stacks push --commits hash1,hash2,hash3   # Push specific commits
csc stacks push --squash 4                    # Squash last 4 commits into 1 clean commit
csc stacks push --squash-since HEAD~5         # Squash all commits since reference
csc stacks submit                             # Submit all unsubmitted entries
csc stacks submit --range 1-3                 # Submit entries 1 through 3
csc stacks submit --range 2,4,6               # Submit specific entries
```

## 🔧 **Git Hooks (Recommended)**

Cascade CLI provides Git hooks that automate common stacked diff workflows:

| Hook Name | Purpose | When It Runs |
|-----------|---------|--------------| 
| `post-commit` | Auto-add commits to active stack with unique branch names | After every `git commit` |
| `pre-push` | Prevent force pushes, validate stack state | Before `git push` |
| `commit-msg` | Validate commit message format | During `git commit` |
| `prepare-commit-msg` | Add stack context to commit messages | Before commit message editor |

```bash
# Install all hooks for automated workflow
csc hooks install

# Remove all hooks 
csc hooks uninstall

# Check installation status
csc hooks status
```

**🪝 Default hooks installed:**
- **Pre-push**: Prevents force pushes that break stack integrity
- **Commit-msg**: Validates commit message quality  
- **Prepare-commit-msg**: Adds helpful stack context

**📚 [Complete Hooks Guide](./docs/HOOKS.md)** - Detailed explanation of each hook, troubleshooting, and advanced usage

**💡 Pro tip**: The post-commit hook (auto-add commits to stack) is available but not installed by default to avoid conflicts with existing repo hooks.

```bash
# Install optional post-commit hook if no conflicts
csc hooks install post-commit
```

**💡 Installation Tips:**
- **For full automation**: Install all hooks with `csc hooks install`
- **For manual control**: Remove `post-commit` hook, keep others for safety
- **For team safety**: Always keep `pre-push` to prevent stack corruption

---

### **4. Experience the Magic**

```bash
# Check your stack status
csc stack
# Stack: feature-auth (3 entries)
# Entry 1: [abc123] Add authentication endpoints → PR #101
# Entry 2: [def456] Add password validation → PR #102  
# Entry 3: [ghi789] Add comprehensive tests → PR #103

# Monitor and auto-merge approved PRs
csc autoland
# ✅ Monitoring PRs for approval + passing builds
# ✅ Will auto-merge in dependency order when ready
```

### **🛡️ Safe Development Flow (Recommended)**

Cascade CLI protects against accidentally polluting your base branch:

```bash
# ✅ SAFE: Start on base branch, but work on feature branches
git checkout main
csc stacks create my-feature --base main

# Make your changes
git checkout -b feature/auth-system  # Create feature branch
git commit -am "Add user authentication"
git commit -am "Add password validation"

# Push to stack (automatically tracks source branch)
csc push  # Adds all unpushed commits to stack with source tracking
```

### **🚀 Auto-Branch Creation (Even Safer)**

Let Cascade CLI handle branch creation automatically:

```bash
# If you accidentally work on main...
git checkout main
# (make commits directly on main - oops!)

# Cascade CLI will protect you:
csc push --auto-branch  # Creates feature branch & moves commits safely
```

### **🔍 Scattered Commit Detection**

Cascade CLI detects when you're adding commits from different Git branches to the same stack and warns you:

```bash
# This creates a "scattered commit" problem:
git checkout feature-branch-1
git commit -m "Add user auth"
git checkout feature-branch-2  
git commit -m "Add admin panel"
git checkout main

# When you push both to the same stack:
csc push

# ⚠️  WARNING: Scattered Commit Detection
#    You've pushed commits from different branches:
#    - feature-branch-1 (1 commit)
#    - feature-branch-2 (1 commit)
#    
#    This makes branch cleanup confusing after merge.
#    Consider organizing commits into separate stacks instead.
```

### **📝 Smart PR Creation**

Cascade CLI automatically generates meaningful pull request titles and descriptions:

```bash
# Create draft PRs for review:
csc submit --draft

# Each PR gets intelligent metadata:
# ┌─ PR Title: Generated from commit messages
# ├─ Description: Combines commit details & context  
# ├─ Branch: Auto-created with semantic naming
# └─ Target: Points to previous stack entry or base

# Custom titles and descriptions:
csc submit 2 --title "Add advanced user auth" --description "Implements JWT tokens with refresh capabilities"

# Default behavior (auto-generated):
csc submit  # Creates well-structured PRs
```

**How PR Content is Generated:**
- **Title**: Uses your commit message or most significant change
- **Description**: Includes commit details, file changes, and stack context
- **Branch Context**: Shows relationship to previous entries
- **Target Branch**: Automatically set to build on previous stack entry

### **Daily Development Flow**

Cascade CLI follows a simple, powerful workflow optimized for modern development:

```bash
# 1. Create & Develop
csc stacks create feature-name --base main
git commit -m "Add core functionality"
git commit -m "Add comprehensive tests"

# 2. Push & Submit (with modern shortcuts)
csc push --squash 2    # Combine commits into reviewable unit
csc submit            # Create PR with automatic dependency tracking

# 3. Auto-Land (set and forget)
csc autoland          # Monitors and merges when approved + tests pass

# 4. Iterate (if review feedback)
git commit --amend   # Update based on feedback
csc sync              # Update all dependent PRs automatically
```

**🔄 Advanced Workflows**: See our comprehensive [**Workflows Guide**](./docs/WORKFLOWS.md) for complex scenarios including:
- Multi-commit stacks with dependencies
- Handling review feedback on middle commits  
- Managing emergency hotfixes during feature development
- Cross-team collaboration patterns
- Base branch updates with smart force push
- Modern WIP-to-clean commit workflows

---

## 🤖 **Smart Conflict Resolution**

### **Automatic Conflict Resolution**

Cascade CLI automatically resolves 60-80% of common rebase conflicts using intelligent pattern recognition.

### **✅ Resolved Automatically**
- **Import statement conflicts** - Merges and deduplicates imports
- **Dependency version conflicts** - Uses latest compatible versions  
- **Simple formatting conflicts** - Applies consistent code style
- **Non-overlapping changes** - Safely combines independent modifications

### **How It Works**
- **Pattern Recognition**: Analyzes conflict types using AST parsing
- **Safe Resolution**: Only resolves conflicts with zero ambiguity
- **Manual Fallback**: Escalates complex conflicts to developer review
- **Audit Trail**: Logs all automatic resolutions for transparency

```bash
# Smart conflict resolution in action
csc rebase
# 🤖 Auto-resolved 3 import conflicts in src/auth.rs
# 🤖 Auto-resolved 1 dependency conflict in package.json
# ⚠️  Manual resolution needed: 1 logic conflict in src/validation.rs
```

### **Supported File Types for Import Resolution**
- **JavaScript/TypeScript** (`.js`, `.ts`, `.jsx`, `.tsx`)
- **Python** (`.py`)
- **Rust** (`.rs`)
- **Swift** (`.swift`) 
- **Kotlin** (`.kt`)
- **C#** (`.cs`)

### **Benefits**
- **Faster rebases** - No manual intervention for simple conflicts
- **Consistent results** - Deterministic conflict resolution
- **Reduced errors** - Eliminates common merge mistakes
- **Learning system** - Improves resolution patterns over time

### **Configuration**
```bash
# Enable/disable smart resolution
csc config set conflicts.auto_resolve true
csc config set conflicts.file_types "js,ts,py,rs"
csc config set conflicts.backup_on_resolve true
```

---

## ⚡ **Essential Commands**

```bash
# Stack Management (Multiple Stacks)
csc stacks create <name> --base <branch>  # Create new stack
csc stacks list                          # List all stacks  
csc stacks switch <name>                 # Switch active stack
csc stacks push                          # Add commits to current stack
csc stacks submit                        # Submit PRs for current stack

# Current Stack Operations (Shortcuts)
csc stack                                # Show current stack details
csc push                                 # Shortcut for 'stacks push'
csc land                                 # Merge approved PRs
csc autoland                             # Auto-merge all ready PRs
csc sync                                 # Sync with remote repository
csc rebase                               # Rebase stack on updated base

# Branch Management & Safety (Now with shortcuts!)
csc deactivate                           # Turn off stack mode (shortcut)
csc switch <name>                        # Switch to different stack (shortcut)

# Or use full commands:
csc stacks deactivate                    # Turn off stack mode (use normal Git workflow)
csc stacks switch <name>                 # Switch to different stack

# 🔍 Automatic branch change detection: Cascade detects when you switch branches
#     and prompts you to continue with current stack, deactivate, or switch stacks

# Repository Overview
csc repo                                 # Show all stacks and repository status
```

**💡 Pro tip**: Use `csc stack` for current stack details, `csc repo` for everything overview.

---

## 📖 **Command Reference**

### **Stack Management**
```bash
# Create and manage stacks
csc stacks create <name>                       # Create new stack (uses default base branch)
csc stacks create <name> --base <branch>       # Create stack with specific base branch
csc stacks create <name> -b <branch>           # Short form
csc stacks create <name> --description <desc>  # Add description
csc stacks create <name> -d <desc>             # Short form

# List stacks
csc stacks list                                # Show basic stack list
csc stacks list --verbose                      # Show detailed information
csc stacks list -v                             # Short form
csc stacks list --active                       # Show only active stack
csc stacks list --format <format>              # Custom output format

# Switch and view stacks
csc stacks switch <name>                       # Activate stack
csc stack                                      # Show current stack details
csc stacks delete <name>                       # Remove stack
csc stacks delete <name> --force               # Force deletion without confirmation
csc stacks validate                            # Validate active stack
csc stacks validate <name>                     # Validate specific stack
```

### **Adding Commits to Stack**
```bash
# Basic push operations
csc stacks push                               # Add current commit (HEAD) to stack
csc stacks push --branch <name>               # Custom branch name for this commit
csc stacks push -b <name>                     # Short form
csc stacks push --message <msg>               # Custom commit message
csc stacks push -m <msg>                      # Short form  
csc stacks push --commit <hash>               # Push specific commit instead of HEAD

# Batch operations
csc stacks push                               # Push all unpushed commits separately
csc stacks push --since HEAD~3                # Push commits since reference
csc stacks push --commits hash1,hash2,hash3   # Push specific commits

# Smart squash operations
csc stacks push --squash 4                    # Squash last 4 commits into 1 clean commit
csc stacks push --squash-since HEAD~5         # Squash all commits since reference

# Remove from stack
csc stacks pop                                # Remove top entry from stack
csc stacks pop --keep-branch                  # Keep the branch when popping
```

### **Pull Request Workflow**
```bash
# Submit for review
csc stacks submit                             # Submit top entry (creates PR)
csc stacks submit 2                           # Submit specific entry number
csc stacks submit --title <title>             # Custom PR title
csc stacks submit -t <title>                  # Short form
csc stacks submit --description <desc>        # Custom PR description
csc stacks submit -d <desc>                   # Short form

# Batch submission
csc stacks submit                             # Submit all unsubmitted entries
csc stacks submit --range 1-3                 # Submit entries 1 through 3
csc stacks submit --range 2,4,6               # Submit specific entries

# Status and management
csc stacks status                             # Show active stack PR status
csc stacks status <name>                      # Show specific stack PR status
csc stacks prs                                # List all repository PRs
csc stacks prs --state open                   # Filter by state (open/merged/declined)
csc stacks prs --verbose                      # Show detailed PR information
csc stacks prs -v                             # Short form
```

### **Sync and Rebase Operations**
```bash
# Sync with remote
csc stacks sync                               # Sync active stack with remote
csc stacks sync --force                       # Force sync even with conflicts

# Rebase operations
csc stacks rebase                             # Rebase stack on latest base branch
csc stacks rebase --interactive               # Interactive rebase mode
csc stacks rebase -i                          # Short form
csc stacks rebase --onto <branch>             # Rebase onto different target branch
csc stacks rebase --strategy cherry-pick      # Use specific rebase strategy
csc stacks rebase --strategy three-way-merge  # Alternative strategies available
csc stacks rebase --no-auto-resolve           # Disable smart conflict resolution

# Rebase conflict resolution
csc stacks continue-rebase                    # Continue after resolving conflicts
csc stacks abort-rebase                       # Abort rebase operation
csc stacks rebase-status                      # Show rebase status and guidance
```

### **Advanced Tools**
```bash
# Interactive interfaces
csc tui                                      # Launch terminal user interface

# Visualization and diagramming
csc viz stack                                # ASCII stack diagram for active stack
csc viz stack <name>                         # ASCII diagram for specific stack
csc viz deps                                 # Show dependency relationships
csc viz deps --format mermaid                # Export as Mermaid diagram
csc viz deps --format dot                    # Export as Graphviz DOT
csc viz deps --format plantuml               # Export as PlantUML
csc viz deps --output <file>                 # Save to file

# Git hooks integration
csc hooks install                            # Install all Git hooks
csc hooks uninstall                          # Remove all Git hooks
csc hooks status                             # Show hook installation status

# Individual hook management
csc hooks add post-commit                    # Install specific hook
csc hooks add pre-push                       # Install push protection  
csc hooks add commit-msg                     # Install commit message validation
csc hooks add prepare-commit-msg             # Install commit message enhancement
csc hooks remove post-commit                 # Remove specific hook
csc hooks remove pre-push                    # Remove push protection
csc hooks remove commit-msg                  # Remove message validation
csc hooks remove prepare-commit-msg          # Remove message enhancement

# Configuration and setup
csc setup                                    # Interactive setup: config + hooks + completions
csc completions install                      # Install shell completions
csc completions status                       # Check completion status
csc completions generate bash                # Generate completions for bash
csc completions generate zsh                 # Generate completions for zsh
csc completions generate fish                # Generate completions for fish

# System information
csc version                                  # Show version information
csc doctor                                   # Run system diagnostics
```

---

## 🔧 **Configuration**

### **Bitbucket Setup**
```bash
# Interactive wizard (recommended)
csc setup

# Manual configuration
csc config set bitbucket.url "https://bitbucket.company.com"
csc config set bitbucket.project "PROJECT"
csc config set bitbucket.repository "repo-name"
csc config set bitbucket.token "your-personal-access-token"
```

---

## 🎨 **Advanced Features**

### **Terminal User Interface**
Launch `csc tui` for an interactive stack browser with:
- Real-time stack visualization
- Keyboard navigation (↑/↓/Enter/q)
- Live status updates
- Error handling and recovery

### **Visualization Export**
```bash
# Generate diagrams for documentation
csc viz stack --format mermaid --output docs/stack.md
csc viz deps --format dot --output diagrams/deps.dot

# Include in CI/CD pipeline
csc viz deps --format plantuml | plantuml -pipe > architecture.png
```

### **Shell Integration**
```bash
# Install completions
csc completions install

# Check installation
csc completions status

# Manual installation
csc completions generate bash > /etc/bash_completion.d/csc
```

---

## 📝 **Documentation**

- 📚 **[User Manual](./docs/USER_MANUAL.md)** - Complete command reference
- 🚀 **[Installation Guide](./docs/INSTALLATION.md)** - Platform-specific instructions
- 🎓 **[Onboarding Guide](./docs/ONBOARDING.md)** - Step-by-step tutorial
- 🔧 **[Configuration Reference](./docs/CONFIGURATION.md)** - All settings explained
- 🐛 **[Troubleshooting](./docs/TROUBLESHOOTING.md)** - Common issues and solutions
- 🏗️ **[Architecture](./docs/ARCHITECTURE.md)** - Internal design and extending
- 📋 **[Smart Force Push Strategy](./docs/EDIT_FLOWS_INTEGRATION.md)** - How PR history is preserved
- 🛠️ **[Development Scripts](./docs/SCRIPTS.md)** - CI debugging and development tools

---

## 🤝 **Contributing**

We welcome contributions! See our [Contributing Guide](./docs/CONTRIBUTING.md) for details.

### **Development Setup**
```bash
git clone https://github.com/JAManfredi/cascade-cli.git
cd cascade-cli
cargo build
cargo test
```

### **Release Process**
See [Release Guide](./docs/RELEASING.md) for maintainer instructions.

---

## 🔧 **Development**

### **Quick Development Setup**

```bash
# Clone and build
git clone https://github.com/jared/cascade-cli.git
cd cascade-cli
cargo build
```

### **Pre-Push Validation**

**Always validate before pushing to GitHub!** Run our comprehensive check script:

```bash
./scripts/pre-push-check.sh
```

This runs all the same checks as CI:
- ✅ Code formatting and linting
- ✅ Unit and integration tests  
- ✅ Documentation generation
- ✅ Binary compilation

**💡 Pro Tip**: Set up a git hook to run this automatically:
```bash
# Add to .git/hooks/pre-push
#!/bin/sh
./scripts/pre-push-check.sh
```

### **CI Debugging Tools**

Having trouble with CI failures that don't reproduce locally? We have specialized tools for that:

📋 **[Development Scripts Guide](./docs/SCRIPTS.md)** - Complete reference for all CI debugging and development scripts

See [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) for complete development guidelines, testing strategies, and contribution workflows.

---

## 🏗️ **Architecture**

Cascade CLI is built with:
- **🦀 Rust** - Performance, safety, and reliability
- **📚 git2** - Native Git operations without subprocess overhead
- **🌐 HTTP/REST** - Direct Bitbucket API integration  
- **🎨 TUI Libraries** - Rich terminal interfaces (ratatui, crossterm)
- **⚡ Async** - Non-blocking operations with tokio

### **Design Principles**
- **Smart Force Push** - Preserves review history while enabling safe rebases
- **Atomic Operations** - All-or-nothing state changes
- **Conflict Detection** - Early detection with resolution guidance
- **Graceful Degradation** - Continue working when services are unavailable

---

## 🧪 **Testing**

```bash
# Run full test suite
cargo test -- --test-threads=1

# Tests cover:
# - Core stack management (40 tests)
# - Git operations and safety
# - Bitbucket integration  
# - CLI command functionality
# - Error handling and recovery
```

**Test Coverage**: 98 tests passing ✅

---

## 📜 **License**

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

<p align="center">
  <strong>📚 Transform your Git workflow with Cascade CLI</strong><br>
  <em>Stack management for modern development teams</em>
</p>

