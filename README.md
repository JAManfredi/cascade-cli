<div align="center">

![Cascade CLI Banner](./assets/banner.svg)

[![Rust](https://img.shields.io/badge/rust-1.82%2B-orange.svg)](https://rustup.rs/)
[![CI](https://github.com/JAManfredi/cascade-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/JAManfredi/cascade-cli/actions/workflows/ci.yml)
[![Production Ready](https://img.shields.io/badge/production-ready-green.svg)](./PRODUCTION_CHECKLIST.md)
[![Tests](https://img.shields.io/badge/tests-50%20passing-brightgreen.svg)](#testing)

</div>

Cascade CLI revolutionizes Git workflows by enabling **stacked diffs** - a powerful technique for managing chains of related commits as separate, reviewable pull requests. Perfect for feature development, bug fixes, and complex changes that benefit from incremental review.

## 📋 Table of Contents

- [✨ Key Features](#key-features)
- [🌿 How Stacked Diffs Work: Branch Management](#how-stacked-diffs-work-branch-management)
- [🚀 Quick Start](#quick-start)
  - [1. Installation](#1-installation)
  - [2. Initialize Your Repository](#2-initialize-your-repository)
  - [3. Create Your First Stack](#3-create-your-first-stack)
  - [🚀 Command Shortcuts](#command-shortcuts)
  - [4. Experience the Magic](#4-experience-the-magic)
- [🔧 Git Hooks (Recommended)](#git-hooks-recommended)
- [🤖 Smart Conflict Resolution](#smart-conflict-resolution)
- [🎯 Core Workflow](#core-workflow)
  - [🛡️ Safe Development Flow (Recommended)](#safe-development-flow-recommended)
  - [🚀 Auto-Branch Creation (Even Safer)](#auto-branch-creation-even-safer)
  - [🔍 Scattered Commit Detection](#scattered-commit-detection)
  - [📝 Smart PR Creation](#smart-pr-creation)
  - [Daily Development Flow](#daily-development-flow)
- [📖 Command Reference](#command-reference)
- [🔧 Configuration](#configuration)
- [🎨 Advanced Features](#advanced-features)
- [🏗️ Architecture](#architecture)
- [🧪 Testing](#testing)
- [🤝 Contributing](#contributing)
- [🔧 Development](#development)
- [📝 Documentation](#documentation)
- [📜 License](#license)
- [🌟 Why Stacked Diffs?](#why-stacked-diffs)

## ✨ **Key Features**

### 🔄 **Stacked Diff Workflow**
- **Chain related commits** into logical, reviewable stacks
- **Independent PR reviews** while maintaining dependencies  
- **Automatic rebase management** when dependencies change
- **Smart force-push strategy** to preserve review history
- **Smart conflict resolution** - Auto-resolves 60-80% of common rebase conflicts

### 🏢 **Enterprise Integration**
- **Bitbucket Server/Cloud** native integration
- **Pull request automation** with dependency tracking
- **Team workflow enforcement** via Git hooks
- **Progress tracking** with real-time status updates

### 🖥️ **Professional Interface**
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
cc stack create feature-auth --base main

# Make commits as usual
git commit -m "Add user authentication endpoints"
git commit -m "Add password validation logic"  
git commit -m "Add comprehensive auth tests"

# Push to stack - Cascade CLI creates separate branches automatically:
cc stack push  # → Creates: add-user-authentication-endpoints
cc stack push  # → Creates: add-password-validation-logic  
cc stack push  # → Creates: add-comprehensive-auth-tests

# Submit creates individual PRs to main:
cc stack submit  # PR #101: add-user-authentication-endpoints → main
cc stack submit  # PR #102: add-password-validation-logic → main (depends on #101)
cc stack submit  # PR #103: add-comprehensive-auth-tests → main (depends on #102)
```

### **Two Workflow Options:**

#### **Option 1: Work on Main (Recommended for Solo Development)**
```bash
git checkout main
cc stack create feature-name --base main
# Make commits directly on main (they stay local until you push to remote)
# Cascade CLI handles all branch creation and PR management
```

#### **Option 2: Work on Feature Branch (Team-Friendly)**  
```bash
git checkout -b feature-work
cc stack create feature-name --base main
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

## 🚀 **Quick Start**

### **1. Installation**

#### **Quick Install (Recommended)**

**Universal Script (Linux/macOS):**
```bash
curl -fsSL https://raw.githubusercontent.com/JAManfredi/cascade-cli/master/install.sh | bash
```

**Package Managers:**
```bash
# macOS - Homebrew
brew install JAManfredi/cascade-cli/cascade-cli

# Rust users
cargo install cascade-cli
```

#### **Manual Installation**

**Pre-built Binaries:**
```bash
# macOS (auto-detect architecture)
curl -L https://github.com/JAManfredi/cascade-cli/releases/latest/download/cc-macos-$(uname -m | sed 's/x86_64/x64/;s/arm64/arm64/').tar.gz | tar -xz
sudo mv cc /usr/local/bin/

# Linux (auto-detect architecture)  
curl -L https://github.com/JAManfredi/cascade-cli/releases/latest/download/cc-linux-$(uname -m | sed 's/x86_64/x64/;s/aarch64/arm64/').tar.gz | tar -xz
sudo mv cc /usr/local/bin/

# Windows (PowerShell)
Invoke-WebRequest -Uri "https://github.com/JAManfredi/cascade-cli/releases/latest/download/cc-windows-x64.exe.zip" -OutFile "cc.zip"
Expand-Archive -Path "cc.zip" -DestinationPath "$env:USERPROFILE\bin\"
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
cc setup

# Or manual initialization
cc init --bitbucket-url https://bitbucket.company.com
```

### **3. Create Your First Stack**
```bash
# Create a new stack
cc stack create feature-auth --base develop --description "User authentication system"

# Make multiple incremental commits (for your own tracking)
git add . && git commit -m "WIP: start authentication"
git add . && git commit -m "WIP: add login logic"
git add . && git commit -m "WIP: fix validation bugs"
git add . && git commit -m "Final: complete auth with tests"

# 🎉 NEW: SQUASH + PUSH - Combine incremental commits into clean commit!
cc stack push --squash 4  # Squashes last 4 commits into 1

# OR: Make some commits normally, then squash later ones
git commit -m "Add core authentication logic"
cc stack push  # Push first clean commit

git commit -m "WIP: start tests"
git commit -m "WIP: more tests"  
git commit -m "Final: comprehensive test suite"

# 🎉 SQUASH UNPUSHED - Only squash the last 3 commits
cc stack push --squash 3  # Squashes and pushes as second stack entry

# 🎉 BATCH SUBMIT - Submit all unsubmitted entries as separate PRs!
cc stack submit

# Alternative options (for granular control):
cc stack push                                   # Push all unpushed commits separately (default)
cc stack push --squash-since HEAD~5             # Squash all commits since HEAD~5
cc stack submit --range 1-3                     # Submit entries 1 through 3
```

### **🚀 Command Shortcuts** 

For frequently used commands, you can skip the `stack` keyword for faster typing:

```bash
# Full commands (always available)
cc stack show           # Show current stack status
cc stack push           # Add commits to stack
cc stack land           # Merge approved PRs
cc stack autoland       # Auto-merge all ready PRs
cc stack sync           # Sync with remote repository
cc stack rebase         # Rebase stack on updated base

# Shortcuts (same functionality, faster typing)
cc show                 # Shortcut for 'stack show'
cc push                 # Shortcut for 'stack push'
cc land                 # Shortcut for 'stack land'
cc autoland             # Shortcut for 'stack autoland'
cc sync                 # Shortcut for 'stack sync'
cc rebase               # Shortcut for 'stack rebase'
```

**💡 Pro tip**: Use shortcuts for daily workflows, full commands for scripts and documentation.

### **4. Experience the Magic**

```bash
# Check your stack status (using shortcuts!)
cc show
# Stack: feature-auth (3 entries)
# Entry 1: [abc123] Add authentication endpoints → PR #101
# Entry 2: [def456] Add password validation → PR #102  
# Entry 3: [ghi789] Add comprehensive tests → PR #103

# Monitor and auto-merge approved PRs
cc autoland
# ✅ Monitoring PRs for approval + passing builds
# ✅ Will auto-merge in dependency order when ready
```

### **🛡️ Safe Development Flow (Recommended)**

Cascade CLI protects against accidentally polluting your base branch:

```bash
# ✅ SAFE: Start on base branch, but work on feature branches
git checkout main
cc stack create my-feature --base main

# Make your changes
git checkout -b feature/auth-system  # Create feature branch
git commit -am "Add user authentication"
git commit -am "Add password validation"

# Push to stack (automatically tracks source branch)
cc push  # Adds all unpushed commits to stack with source tracking
```

### **🚀 Auto-Branch Creation (Even Safer)**

Let Cascade CLI handle branch creation automatically:

```bash
# If you accidentally work on main...
git checkout main
# (make commits directly on main - oops!)

# Cascade CLI will protect you:
cc push --auto-branch  # Creates feature branch & moves commits safely
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
cc push --all

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
cc submit --all --draft

# Each PR gets intelligent metadata:
# ┌─ PR Title: Generated from commit messages
# ├─ Description: Combines commit details & context  
# ├─ Branch: Auto-created with semantic naming
# └─ Target: Points to previous stack entry or base

# Custom titles and descriptions:
cc submit 2 --title "Add advanced user auth" --description "Implements JWT tokens with refresh capabilities"

# Default behavior (auto-generated):
cc submit --all  # Creates production-ready PRs
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
cc stack create feature-name --base main
git commit -m "Add core functionality"
git commit -m "Add comprehensive tests"

# 2. Push & Submit (with modern shortcuts)
cc push --squash 2    # Combine commits into reviewable unit
cc submit            # Create PR with automatic dependency tracking

# 3. Auto-Land (set and forget)
cc autoland          # Monitors and merges when approved + tests pass

# 4. Iterate (if review feedback)
git commit --amend   # Update based on feedback
cc sync              # Update all dependent PRs automatically
```

**🔄 Advanced Workflows**: See our comprehensive [**Workflows Guide**](./docs/WORKFLOWS.md) for complex scenarios including:
- Multi-commit stacks with dependencies
- Handling review feedback on middle commits  
- Managing emergency hotfixes during feature development
- Cross-team collaboration patterns
- Base branch updates with smart force push
- Modern WIP-to-clean commit workflows

---

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
cc hooks install

# Remove all hooks 
cc hooks uninstall

# Check installation status
cc hooks status

# Individual hook management
cc hooks add post-commit        # Enable auto-stack management
cc hooks add pre-push           # Enable push validation
cc hooks add commit-msg         # Enable message validation
cc hooks add prepare-commit-msg # Enable message enhancement

cc hooks remove post-commit     # Disable auto-stack (manual control)
```

**💡 Installation Tips:**
- **For full automation**: Install all hooks with `cc hooks install`
- **For manual control**: Remove `post-commit` hook, keep others for safety
- **For team safety**: Always keep `pre-push` to prevent stack corruption

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
cc rebase
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
cc config set conflicts.auto_resolve true
cc config set conflicts.file_types "js,ts,py,rs"
cc config set conflicts.backup_on_resolve true
```

---

## 🎯 **Core Workflow**

**💡 First time using stacked diffs?** Read about [Git Branches vs Stacks](docs/WORKFLOWS.md#understanding-git-branches-vs-stacks) to understand how they work together.

### **🛡️ Safe Development Flow (Recommended)**

Cascade CLI protects against accidentally polluting your base branch:

```bash
# ✅ SAFE: Start on base branch, but work on feature branches
git checkout main
cc stack create my-feature --base main

# Make your changes
git checkout -b feature/auth-system  # Create feature branch
git commit -am "Add user authentication"
git commit -am "Add password validation"

# Push to stack (automatically tracks source branch)
cc push --all  # Adds commits to stack with source tracking
```

### **🚀 Auto-Branch Creation (Even Safer)**

Let Cascade CLI handle branch creation automatically:

```bash
# If you accidentally work on main...
git checkout main
# (make commits directly on main - oops!)

# Cascade CLI will protect you:
cc push --auto-branch  # Creates feature branch & moves commits safely
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
cc push --all

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
cc submit --all --draft

# Each PR gets intelligent metadata:
# ┌─ PR Title: Generated from commit messages
# ├─ Description: Combines commit details & context  
# ├─ Branch: Auto-created with semantic naming
# └─ Target: Points to previous stack entry or base

# Custom titles and descriptions:
cc submit 2 --title "Add advanced user auth" --description "Implements JWT tokens with refresh capabilities"

# Default behavior (auto-generated):
cc submit --all  # Creates production-ready PRs
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
cc stack create feature-name --base main
git commit -m "Add core functionality"
git commit -m "Add comprehensive tests"

# 2. Push & Submit (with modern shortcuts)
cc push --squash 2    # Combine commits into reviewable unit
cc submit            # Create PR with automatic dependency tracking

# 3. Auto-Land (set and forget)
cc autoland          # Monitors and merges when approved + tests pass

# 4. Iterate (if review feedback)
git commit --amend   # Update based on feedback
cc sync              # Update all dependent PRs automatically
```

**🔄 Advanced Workflows**: See our comprehensive [**Workflows Guide**](./docs/WORKFLOWS.md) for complex scenarios including:
- Multi-commit stacks with dependencies
- Handling review feedback on middle commits  
- Managing emergency hotfixes during feature development
- Cross-team collaboration patterns
- Base branch updates with smart force push
- Modern WIP-to-clean commit workflows

---

## 📖 **Command Reference**

### **Stack Management**
```bash
# Create and manage stacks
cc stack create <name>                       # Create new stack (uses default base branch)
cc stack create <name> --base <branch>       # Create stack with specific base branch
cc stack create <name> -b <branch>           # Short form
cc stack create <name> --description <desc>  # Add description
cc stack create <name> -d <desc>             # Short form

# List stacks
cc stack list                                # Show basic stack list
cc stack list --verbose                      # Show detailed information
cc stack list -v                             # Short form
cc stack list --active                       # Show only active stack
cc stack list --format <format>              # Custom output format

# Switch and view stacks
cc stack switch <name>                       # Activate stack
cc stack show                                # Show active stack details
cc stack show <name>                         # Show specific stack details
cc stack delete <name>                       # Remove stack
cc stack delete <name> --force               # Force deletion without confirmation
cc stack validate                            # Validate active stack
cc stack validate <name>                     # Validate specific stack
```

### **Adding Commits to Stack**
```bash
# Basic push operations
cc stack push                               # Add current commit (HEAD) to stack
cc stack push --branch <name>               # Custom branch name for this commit
cc stack push -b <name>                     # Short form
cc stack push --message <msg>               # Custom commit message
cc stack push -m <msg>                      # Short form  
cc stack push --commit <hash>               # Push specific commit instead of HEAD

# Batch operations
cc stack push --all                         # 🎉 Push all unpushed commits separately
cc stack push --since HEAD~3                # 🎉 Push commits since reference
cc stack push --commits hash1,hash2,hash3   # 🎉 Push specific commits

# Smart squash operations
cc stack push --squash 4                    # 🎉 Squash last 4 commits into 1 clean commit
cc stack push --squash-since HEAD~5         # 🎉 Squash all commits since reference

# Remove from stack
cc stack pop                                # Remove top entry from stack
cc stack pop --keep-branch                  # Keep the branch when popping
```

### **Pull Request Workflow**
```bash
# Submit for review
cc stack submit                             # Submit top entry (creates PR)
cc stack submit 2                           # Submit specific entry number
cc stack submit --title <title>             # Custom PR title
cc stack submit -t <title>                  # Short form
cc stack submit --description <desc>        # Custom PR description
cc stack submit -d <desc>                   # Short form

# Batch submission
cc stack submit --all                       # 🎉 Submit all unsubmitted entries
cc stack submit --range 1-3                 # 🎉 Submit entries 1 through 3
cc stack submit --range 2,4,6               # 🎉 Submit specific entries

# Status and management
cc stack status                             # Show active stack PR status
cc stack status <name>                      # Show specific stack PR status
cc stack prs                                # List all repository PRs
cc stack prs --state open                   # Filter by state (open/merged/declined)
cc stack prs --verbose                      # Show detailed PR information
cc stack prs -v                             # Short form
```

### **Sync and Rebase Operations**
```bash
# Sync with remote
cc stack sync                               # Sync active stack with remote
cc stack sync --force                       # Force sync even with conflicts

# Rebase operations
cc stack rebase                             # Rebase stack on latest base branch
cc stack rebase --interactive               # Interactive rebase mode
cc stack rebase -i                          # Short form
cc stack rebase --onto <branch>             # Rebase onto different target branch
cc stack rebase --strategy cherry-pick      # Use specific rebase strategy
cc stack rebase --strategy three-way-merge  # Alternative strategies available
cc stack rebase --no-auto-resolve           # Disable smart conflict resolution

# Rebase conflict resolution
cc stack continue-rebase                    # Continue after resolving conflicts
cc stack abort-rebase                       # Abort rebase operation
cc stack rebase-status                      # Show rebase status and guidance
```

### **Advanced Tools**
```bash
# Interactive interfaces
cc tui                                      # Launch terminal user interface

# Visualization and diagramming
cc viz stack                                # ASCII stack diagram for active stack
cc viz stack <name>                         # ASCII diagram for specific stack
cc viz deps                                 # Show dependency relationships
cc viz deps --format mermaid                # Export as Mermaid diagram
cc viz deps --format dot                    # Export as Graphviz DOT
cc viz deps --format plantuml               # Export as PlantUML
cc viz deps --output <file>                 # Save to file

# Git hooks integration
cc hooks install                            # Install all Git hooks
cc hooks uninstall                          # Remove all Git hooks
cc hooks status                             # Show hook installation status

# Individual hook management
cc hooks add post-commit                    # Install specific hook
cc hooks add pre-push                       # Install push protection  
cc hooks add commit-msg                     # Install commit message validation
cc hooks add prepare-commit-msg             # Install commit message enhancement
cc hooks remove post-commit                 # Remove specific hook
cc hooks remove pre-push                    # Remove push protection
cc hooks remove commit-msg                  # Remove message validation
cc hooks remove prepare-commit-msg          # Remove message enhancement

# Configuration and setup
cc setup                                    # Interactive configuration wizard
cc completions install                      # Install shell completions
cc completions status                       # Check completion status
cc completions generate bash                # Generate completions for bash
cc completions generate zsh                 # Generate completions for zsh
cc completions generate fish                # Generate completions for fish

# System information
cc version                                  # Show version information
cc doctor                                   # Run system diagnostics
```

---

## 🔧 **Configuration**

### **Bitbucket Setup**
```bash
# Interactive wizard (recommended)
cc setup

# Manual configuration
cc config set bitbucket.url "https://bitbucket.company.com"
cc config set bitbucket.project "PROJECT"
cc config set bitbucket.repository "repo-name"
cc config set bitbucket.token "your-personal-access-token"
```

### **Git Hooks (Recommended)**

Cascade CLI provides Git hooks that automate common stacked diff workflows:

| Hook Name | Purpose | When It Runs |
|-----------|---------|--------------| 
| `post-commit` | Auto-add commits to active stack with unique branch names | After every `git commit` |
| `pre-push` | Prevent force pushes, validate stack state | Before `git push` |
| `commit-msg` | Validate commit message format | During `git commit` |
| `prepare-commit-msg` | Add stack context to commit messages | Before commit message editor |

```bash
# Install all hooks for automated workflow
cc hooks install

# Remove all hooks 
cc hooks uninstall

# Check installation status
cc hooks status

# Individual hook management
cc hooks add post-commit        # Enable auto-stack management
cc hooks add pre-push           # Enable push validation
cc hooks add commit-msg         # Enable message validation
cc hooks add prepare-commit-msg # Enable message enhancement

cc hooks remove post-commit     # Disable auto-stack (manual control)
```

**💡 Installation Tips:**
- **For full automation**: Install all hooks with `cc hooks install`
- **For manual control**: Remove `post-commit` hook, keep others for safety
- **For team safety**: Always keep `pre-push` to prevent stack corruption

---

## 🎨 **Advanced Features**

### **Terminal User Interface**
Launch `cc tui` for an interactive stack browser with:
- Real-time stack visualization
- Keyboard navigation (↑/↓/Enter/q)
- Live status updates
- Error handling and recovery

### **Visualization Export**
```bash
# Generate diagrams for documentation
cc viz stack --format mermaid --output docs/stack.md
cc viz deps --format dot --output diagrams/deps.dot

# Include in CI/CD pipeline
cc viz deps --format plantuml | plantuml -pipe > architecture.png
```

### **Shell Integration**
```bash
# Install completions
cc completions install

# Check installation
cc completions status

# Manual installation
cc completions generate bash > /etc/bash_completion.d/cc
```

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

**Test Coverage**: 40/40 tests passing ✅

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

## 📝 **Documentation**

- 📚 **[User Manual](./docs/USER_MANUAL.md)** - Complete command reference
- 🚀 **[Installation Guide](./docs/INSTALLATION.md)** - Platform-specific instructions
- 🎓 **[Onboarding Guide](./docs/ONBOARDING.md)** - Step-by-step tutorial
- 🔧 **[Configuration Reference](./docs/CONFIGURATION.md)** - All settings explained
- 🐛 **[Troubleshooting](./docs/TROUBLESHOOTING.md)** - Common issues and solutions
- 🏗️ **[Architecture](./docs/ARCHITECTURE.md)** - Internal design and extending
- 📋 **[Smart Force Push Strategy](./docs/EDIT_FLOWS_INTEGRATION.md)** - How PR history is preserved
- 🚀 **[Upcoming Features](./docs/UPCOMING.md)** - Planned features and roadmap

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

See [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) for complete development guidelines, testing strategies, and contribution workflows.

---

## 📜 **License**

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

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

<p align="center">
  <strong>📚 Transform your Git workflow with Cascade CLI</strong><br>
  <em>Professional stack management for modern development teams</em>
</p>

### Daily Workflow Commands

```bash
# Check if your stack needs syncing (read-only status check)
cc stack check

# Sync with remote changes (pull + rebase + cleanup)  
cc stack sync

# Make changes and push to stack
cc stack push --message "Add feature X"

# Submit for review
cc stack submit --all

# Land completed PRs with auto-retargeting
cc stack land
```