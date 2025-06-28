# 📚 User Manual

Complete reference guide for Cascade CLI commands, workflows, and advanced usage.

## 📖 **Table of Contents**

1. [Core Concepts](#core-concepts)
2. [Command Reference](#command-reference)
3. [Workflow Patterns](#workflow-patterns)
4. [Advanced Usage](#advanced-usage)
5. [Configuration](#configuration)
6. [Troubleshooting](#troubleshooting)

---

## 🧭 **Core Concepts**

### **What is a Stack?**
A **stack** is a logical grouping of related commits that represent incremental progress toward a larger feature or fix. Each commit in the stack can be submitted as a separate pull request while maintaining dependencies.

```
📚 Stack: "user-authentication"
├── Commit 1: "Add login endpoint"        → PR #123
├── Commit 2: "Add password validation"   → PR #124 (depends on #123)
└── Commit 3: "Add password reset"        → PR #125 (depends on #124)
```

### **Key Benefits**
- **Faster Reviews**: Small, focused PRs are easier to review
- **Parallel Development**: Work on multiple features simultaneously
- **Better Quality**: Incremental feedback improves code quality
- **Reduced Conflicts**: Frequent integration prevents merge hell

### **Stack Lifecycle**
1. **Create** - Initialize a new stack with a base branch
2. **Push** - Add commits to the stack
3. **Submit** - Create pull requests for stack entries
4. **Sync** - Update stack when dependencies change
5. **Pop** - Remove completed entries from the stack

---

## 📖 **Command Reference**

### **🎯 Core Commands**

#### **`cc init`** - Initialize Repository
Initialize Cascade CLI in a Git repository.

```bash
cc init [OPTIONS]

# Options:
--bitbucket-url <URL>     # Bitbucket Server URL
--project <PROJECT>       # Project key
--repository <REPO>       # Repository name
--force                   # Overwrite existing configuration
```

**Examples:**
```bash
# Interactive initialization
cc init

# Manual configuration
cc init --bitbucket-url https://bitbucket.company.com --project DEV --repository my-app

# Force reconfiguration
cc init --force
```

#### **`cc setup`** - Interactive Setup Wizard
Guided configuration for first-time users.

```bash
cc setup [OPTIONS]

# Options:
--force                   # Force reconfiguration if already initialized
```

**Features:**
- Auto-detects Git remotes
- Configures Bitbucket settings
- Tests connections
- Installs shell completions
- Validates Personal Access Tokens

### **📚 Stack Management**

#### **`cc stacks create`** - Create New Stack
Create a new stack for organizing related commits.

```bash
cc stacks create <NAME> [OPTIONS]

# Options:
--base <BRANCH>           # Base branch (default: current branch)
--description <DESC>      # Stack description
--activate               # Activate after creation (default: true)
```

**Examples:**
```bash
# Basic stack creation
cc stacks create feature-auth --base develop

# With description
cc stacks create fix-performance --base main --description "Database query optimizations"

# Create without activating
cc stacks create future-feature --base develop --no-activate
```

#### **`cc stacks list`** - List All Stacks
Display all stacks with their status and information.

```bash
cc stacks list [OPTIONS]

# Options:
--verbose, -v            # Show detailed information
--active                 # Show only active stack
--format <FORMAT>        # Output format (name, id, status)
```

**Examples:**
```bash
# Simple list
cc stacks list

# Detailed view
cc stacks list --verbose

# Only active stack
cc stacks list --active

# Custom format
cc stacks list --format status
```

#### **`cc stack`** - Display Stack Details
Show detailed information about a specific stack.

```bash
cc stack [NAME]

# Arguments:
[NAME]                   # Stack name (defaults to active stack)
```

**Output includes:**
- Stack metadata (name, description, base branch)
- All stack entries with commit details
- Pull request status and links
- Dependency information

#### **`cc stacks switch`** - Activate Stack
Switch to a different stack, making it the active stack.

```bash
cc stacks switch <NAME>

# Arguments:
<NAME>                   # Stack name to activate
```

**Examples:**
```bash
cc stacks switch feature-auth
cc stacks switch fix-bugs
```

#### **`cc stacks delete`** - Remove Stack
Delete a stack and optionally its associated branches.

```bash
cc stacks delete <NAME> [OPTIONS]

# Options:
--force                  # Skip confirmation prompt
--keep-branches         # Keep associated branches
```

**Examples:**
```bash
# With confirmation
cc stacks delete old-feature

# Force deletion
cc stacks delete temp-stack --force

# Delete but keep branches
cc stacks delete feature-x --keep-branches
```

### **📤 Stack Operations**

#### **`cc stacks push`** - Add Commits to Stack
Add commits to the active stack. By default, pushes all unpushed commits.

```bash
cc stacks push [OPTIONS]

# Options:
--branch <NAME>         # Custom branch name for this commit
--message <MSG>         # Commit message (if creating new commit)
--commit <HASH>         # Use specific commit instead of HEAD
--since <REF>           # Push commits since reference (e.g., HEAD~3)
--commits <HASHES>      # Push specific commits (comma-separated)
--squash <N>            # 🎉 Squash last N commits into 1 clean commit
--squash-since <REF>    # 🎉 Squash all commits since reference
```

**Default Behavior:** When no specific targeting options are provided, `cc stacks push` pushes **all unpushed commits** since the last stack push.

**Squash Workflow Examples:**
```bash
# Make incremental commits during development
git commit -m "WIP: start feature"
git commit -m "WIP: add core logic"
git commit -m "WIP: fix bugs"
git commit -m "Final: complete feature with tests"

# 🔍 See unpushed commits and get squash suggestions
cc stack
# 🚧 Unpushed commits (4): use 'cc stacks push --squash 4' to squash them
#    1. WIP: start feature (abc123)
#    2. WIP: add core logic (def456)
#    3. WIP: fix bugs (ghi789)
#    4. Final: complete feature with tests (jkl012)
# 💡 Squash options:
#    cc stacks push --squash 4           # Squash all unpushed commits
#    cc stacks push --squash 3           # Squash last 3 commits only

# 🎉 Smart squash automatically detects "Final:" commits and creates intelligent messages
cc stacks push --squash 4
# ✅ Smart message: Complete feature with tests (automatically extracted from "Final:" commit)

# Alternative patterns that smart squash recognizes:
git commit -m "WIP: authentication work"
git commit -m "Add user authentication with OAuth"  # Uses this descriptive message
cc stacks push --squash 2  # Result: "Add user authentication with OAuth"

git commit -m "fix typo"
git commit -m "fix bug"  
git commit -m "refactor cleanup"
cc stacks push --squash 3  # Result: "Refactor cleanup" (uses last commit)
```

**Branch Naming:** Generated from final squashed commit message using Cascade CLI's branch naming rules.

**Examples:**
```bash
# Push all unpushed commits (default behavior)
git commit -m "Add user authentication"
git commit -m "Add password validation"
cc stacks push  # Pushes both commits as separate stack entries

# Push specific commit only
cc stacks push --commit abc123

# Push commits since specific reference
cc stacks push --since HEAD~3

# Push specific commits
cc stacks push --commits abc123,def456,ghi789

# Push with custom branch name
cc stacks push --branch custom-auth-branch

# Squash multiple commits before pushing
cc stacks push --squash 3  # Squashes last 3 commits into one

# Squash commits since reference
cc stacks push --squash-since HEAD~5
```

#### **`cc stacks pop`** - Remove Entry from Stack
Remove the top entry from the stack.

```bash
cc stacks pop [OPTIONS]

# Options:
--keep-branch           # Keep the associated branch
--force                 # Skip confirmation
```

**Examples:**
```bash
# Remove top entry
cc stacks pop

# Keep the branch
cc stacks pop --keep-branch

# Force removal
cc stacks pop --force
```

#### **`cc stacks submit`** - Create Pull Requests
Submit stack entries as pull requests. By default, submits all unsubmitted entries.

```bash
cc stacks submit [ENTRY] [OPTIONS]

# Arguments:
[ENTRY]                 # Entry index (defaults to all unsubmitted entries)

# Options:
--title <TITLE>         # PR title override
--description <DESC>    # PR description
--range <RANGE>         # Submit range of entries (e.g., "1-3" or "2,4,6")
--draft                 # Create as draft PR
--reviewers <USERS>     # Comma-separated reviewer list
```

**Default Behavior:** When no specific entry is provided, `cc stacks submit` submits **all unsubmitted entries** as separate pull requests.

**Examples:**
```bash
# Submit all unsubmitted entries (default behavior)
cc stacks submit

# Submit specific entry
cc stacks submit 2

# Submit range of entries
cc stacks submit --range 1-3

# Submit specific entries 
cc stacks submit --range 2,4,6

# Submit with custom details
cc stacks submit --title "Add OAuth integration" --description "Implements Google OAuth2 flow"

# Create draft PRs
cc stacks submit --draft

# Add reviewers
cc stacks submit --reviewers "alice,bob,charlie"
```

#### **`cc stacks sync`** - Synchronize with Remote
Update stack with latest changes from base branch and dependencies.

```bash
cc stacks sync [OPTIONS]

# Options:
--force                 # Force sync even with conflicts
--strategy <STRATEGY>   # Sync strategy (merge, rebase, cherry-pick)
```

**Examples:**
```bash
# Standard sync
cc stacks sync

# Force sync with conflicts
cc stacks sync --force

# Use specific strategy
cc stacks sync --strategy rebase
```

#### **`cc stacks rebase`** - Rebase Stack
Rebase all stack entries on latest base branch using smart force push strategy.

```bash
cc stacks rebase [OPTIONS]

# Options:
--interactive          # Interactive rebase mode
--strategy <STRATEGY>  # Rebase strategy (cherry-pick, merge)
--continue            # Continue after resolving conflicts
--abort               # Abort rebase operation
```

**Smart Force Push Behavior:**
When rebasing, Cascade CLI:
1. Creates temporary versioned branches (`feature-v2`)
2. Force pushes new content to original branches (`feature`)
3. **Preserves ALL existing PRs** and review history
4. Keeps versioned branches as backup for safety

This approach follows industry standards (Graphite, Phabricator, GitHub CLI) and ensures reviewers never lose context, comments, or approval history.

**Examples:**
```bash
# Standard rebase with PR history preservation
cc stacks rebase

# Interactive rebase
cc stacks rebase --interactive

# Continue after conflict resolution
cc stacks rebase --continue

# Abort rebase
cc stacks rebase --abort
```

**What you'll see:**
```bash
$ cc stacks rebase

🔄 Rebasing stack: authentication
   📋 Branch mapping:
      add-auth -> add-auth-v2      # Temporary rebase branches
      add-tests -> add-tests-v2

   🔄 Preserved pull request history:
      ✅ Force-pushed add-auth-v2 content to add-auth (preserves PR #123)
      ✅ Force-pushed add-tests-v2 content to add-tests (preserves PR #124)

   ✅ 2 commits successfully rebased
```

### **📊 Status and Information**

#### **`cc repo`** - Show Repository Overview
Display comprehensive status of current repository and stacks.

```bash
cc repo [OPTIONS]

# Options:
--verbose, -v           # Show detailed information
--format <FORMAT>       # Output format (table, json, yaml)
```

**Output includes:**
- Repository status
- Active stack information
- Uncommitted changes
- Pull request status
- Sync status with remotes

#### **`cc stacks status`** - Stack-Specific Status
Show detailed status for current or specified stack.

```bash
cc stacks status [NAME]

# Arguments:
[NAME]                  # Stack name (defaults to active stack)
```

#### **`cc stacks prs`** - List Pull Requests
Show all pull requests associated with stacks.

```bash
cc stacks prs [OPTIONS]

# Options:
--stack <NAME>          # Filter by stack name
--status <STATUS>       # Filter by PR status (open, merged, declined)
--format <FORMAT>       # Output format (table, json)
```

**Examples:**
```bash
# All PRs
cc stacks prs

# PRs for specific stack
cc stacks prs --stack feature-auth

# Only open PRs
cc stacks prs --status open
```

### **🎨 Visualization**

#### **`cc viz stack`** - Stack Diagram
Generate visual representation of a stack.

```bash
cc viz stack [NAME] [OPTIONS]

# Arguments:
[NAME]                  # Stack name (defaults to active stack)

# Options:
--format <FORMAT>       # Output format (ascii, mermaid, dot, plantuml)
--output <FILE>         # Save to file
--compact              # Compact display mode
--no-colors            # Disable colored output
```

**Examples:**
```bash
# ASCII diagram in terminal
cc viz stack

# Mermaid diagram
cc viz stack --format mermaid

# Save to file
cc viz stack --format dot --output stack.dot

# Compact mode
cc viz stack --compact
```

#### **`cc viz deps`** - Dependency Graph
Show dependencies between all stacks.

```bash
cc viz deps [OPTIONS]

# Options:
--format <FORMAT>       # Output format (ascii, mermaid, dot, plantuml)
--output <FILE>         # Save to file
--compact              # Compact display mode
--no-colors            # Disable colored output
```

**Examples:**
```bash
# ASCII dependency graph
cc viz deps

# Export to Mermaid
cc viz deps --format mermaid --output deps.md

# Graphviz format for advanced visualization
cc viz deps --format dot --output deps.dot
```

### **🖥️ Interactive Tools**

#### **`cc tui`** - Terminal User Interface
Launch interactive stack browser.

```bash
cc tui
```

**Features:**
- Real-time stack visualization
- Keyboard navigation (↑/↓/Enter/q/r)
- Stack activation and switching
- Live status updates
- Error handling and recovery

**Keyboard Controls:**
- `↑/↓` - Navigate stacks
- `Enter` - Activate selected stack
- `r` - Refresh data
- `q` - Quit

### **🪝 Git Hooks**

#### **`cc hooks install`** - Install All Hooks
Install all Cascade Git hooks for workflow automation.

```bash
cc hooks install [OPTIONS]

# Options:
--force                 # Overwrite existing hooks
```

#### **`cc hooks uninstall`** - Remove All Hooks
Remove all Cascade Git hooks.

```bash
cc hooks uninstall
```

#### **`cc hooks status`** - Show Hook Status
Display installation status of all Git hooks.

```bash
cc hooks status
```

#### **`cc hooks add`** - Install Specific Hook
Install a specific Git hook.

```bash
cc hooks add <HOOK>

# Hook types:
post-commit            # Auto-add commits to active stack
pre-push              # Prevent dangerous pushes to protected branches
commit-msg            # Validate commit message format
prepare-commit-msg    # Add stack context to commit messages
```

#### **`cc hooks remove`** - Remove Specific Hook
Remove a specific Git hook.

```bash
cc hooks remove <HOOK>
```

### **⚙️ Configuration**

#### **`cc config`** - Configuration Management
Manage Cascade CLI configuration settings.

```bash
cc config <SUBCOMMAND>

# Subcommands:
list                   # Show all configuration
get <KEY>             # Get specific value
set <KEY> <VALUE>     # Set configuration value
unset <KEY>           # Remove configuration value
```

**Examples:**
```bash
# List all configuration
cc config list

# Get specific setting
cc config get bitbucket.url

# Set configuration
cc config set bitbucket.token "your-token-here"

# Remove setting
cc config unset bitbucket.project
```

### **🔧 Utility Commands**

#### **`cc doctor`** - System Diagnostics
Run comprehensive system health check.

```bash
cc doctor [OPTIONS]

# Options:
--verbose, -v           # Show detailed diagnostics
--fix                  # Attempt to fix common issues
```

#### **`cc completions`** - Shell Completions
Manage shell completion installation.

```bash
cc completions <SUBCOMMAND>

# Subcommands:
install               # Auto-install for detected shells
status               # Show installation status
generate <SHELL>     # Generate completions for specific shell
```

#### **`cc version`** - Version Information
Display version and build information.

```bash
cc version [OPTIONS]

# Options:
--verbose, -v         # Show detailed build information
```

---

## 🔄 **Workflow Patterns**

### **Feature Development Workflow**

#### **1. Start New Feature**
```bash
# Create feature stack
cc stacks create feature-user-profiles --base develop --description "User profile management system"

# Start development
git checkout develop
git pull origin develop
```

#### **2. Incremental Development**
```bash
# First increment: basic profile model
git add . && git commit -m "Add user profile model"
cc stacks push

# Second increment: profile endpoints
git add . && git commit -m "Add profile CRUD endpoints"
cc stacks push

# Third increment: profile validation
git add . && git commit -m "Add profile data validation"
cc stacks push
```

#### **3. Submit for Review**
```bash
# Submit each increment as separate PRs
cc stacks submit 1  # Submit profile model
cc stacks submit 2  # Submit endpoints (depends on model)
cc stacks submit 3  # Submit validation (depends on endpoints)
```

#### **4. Handle Review Feedback**
```bash
# Make changes to address feedback
git add . && git commit -m "Address review feedback: improve validation"

# Update existing PR
cc stacks submit 3 --title "Updated: Add profile data validation"

# Or sync if dependencies changed
cc stacks sync
```

#### **5. Merge and Clean Up**
```bash
# After PRs are approved and merged
cc stacks pop  # Remove merged entries
cc stacks pop
cc stacks pop

# Or delete completed stack
cc stacks delete feature-user-profiles
```

### **Bug Fix Workflow**

#### **Quick Fix**
```bash
# Create fix stack
cc stacks create fix-login-bug --base main --description "Fix login timeout issue"

# Make fix
git add . && git commit -m "Fix login timeout in OAuth flow"
cc stacks push

# Submit immediately
cc stacks submit --reviewers "security-team"
```

#### **Complex Fix with Investigation**
```bash
# Investigation stack
cc stacks create investigate-memory-leak --base develop

# Add investigation commits
git commit -m "Add memory profiling tools"
cc stacks push

git commit -m "Identify memory leak in cache layer"
cc stacks push

git commit -m "Fix memory leak and add tests"
cc stacks push

# Submit investigation and fix separately
cc stacks submit 1 --title "Add memory profiling tools"
cc stacks submit 3 --title "Fix memory leak in cache layer"
```

### **Team Collaboration Patterns**

#### **Dependent Feature Development**
```bash
# Team member A: Core infrastructure
cc stacks create auth-core --base main
git commit -m "Add OAuth2 infrastructure"
cc stacks push
cc stacks submit

# Team member B: Dependent feature (waits for A's PR)
cc stacks create user-management --base auth-core
git commit -m "Add user management using OAuth2"
cc stacks push

# After A's PR is merged, B syncs
cc stacks sync  # Rebase on latest main including A's changes
cc stacks submit
```

#### **Parallel Development with Coordination**
```bash
# Feature A: Independent
cc stacks create feature-a --base develop
# ... development work ...

# Feature B: Independent
cc stacks create feature-b --base develop  
# ... development work ...

# Visualize dependencies
cc viz deps --format mermaid > team-deps.md
```

---

## 🎯 **Advanced Usage**

### **Custom Workflow Integration**

#### **CI/CD Integration**
```bash
# In CI pipeline
cc doctor --verbose           # Validate environment
cc stacks status --format json # Get status for reporting
cc viz deps --format dot      # Generate dependency graphs
```

#### **Pre-commit Hook Integration**
```bash
# Install hooks for automatic workflow
cc hooks install

# Hooks will automatically:
# - Add commits to active stack
# - Validate commit messages
# - Prevent dangerous operations
```

### **Large Repository Optimization**

#### **Performance Configuration**
```bash
# Optimize for large repos
cc config set performance.cache_size 2000
cc config set performance.parallel_operations true
cc config set network.timeout 120
```

#### **Selective Stack Management**
```bash
# Work with specific stacks only
cc stacks list --format name | grep feature- | xargs -I {} cc stacks validate {}
```

### **Advanced Visualization**

#### **Documentation Generation**
```bash
# Generate project architecture docs
cc viz deps --format mermaid --output docs/architecture.md

# Include in markdown
echo "# Project Architecture" > docs/full-arch.md
echo "## Stack Dependencies" >> docs/full-arch.md
cc viz deps --format mermaid >> docs/full-arch.md
```

#### **Custom Formats for Tools**
```bash
# Export for external tools
cc viz stack --format dot | dot -Tpng > stack-diagram.png
cc viz deps --format plantuml | plantuml -pipe > deps.svg
```

---

## ⚙️ **Configuration**

### **Configuration File Location**
```
~/.cascade/config.toml          # User configuration
./.cascade/config.toml          # Repository configuration (overrides user)
```

### **Complete Configuration Reference**

```toml
[bitbucket]
url = "https://bitbucket.company.com"
project = "PROJECT_KEY"
repository = "repo-name"
token = "your-personal-access-token"

[git]
default_branch = "main"
auto_sync = true
conflict_strategy = "cherry-pick"

[workflow]
auto_submit = false
require_pr_template = true
default_reviewers = ["team-lead", "senior-dev"]

[ui]
colors = true
progress_bars = true
emoji = true

[performance]
cache_size = 1000
parallel_operations = true
timeout = 60

[hooks]
post_commit = true
pre_push = true
commit_msg = true
prepare_commit_msg = false
```

### **Environment Variables**
```bash
CASCADE_CONFIG_DIR="/custom/config/path"
CASCADE_LOG_LEVEL="debug"
BITBUCKET_TOKEN="token-from-env"
BITBUCKET_URL="https://bitbucket.company.com"
```

---

## 🚨 **Troubleshooting**

### **Common Issues and Solutions**

#### **"Stack not found" errors**
```bash
# List all stacks to verify names
cc stacks list

# Check if in correct repository
cc repo

# Re-initialize if needed
cc init --force
```

#### **Bitbucket connection issues**
```bash
# Test connection
cc doctor

# Verify token permissions
cc config get bitbucket.token

# Reconfigure if needed
cc setup --force
```

#### **Sync conflicts**
```bash
# Check conflict status
cc stacks status

# Resolve manually and continue
git add .
cc stacks rebase --continue

# Or abort and try different strategy
cc stacks rebase --abort
cc stacks sync --strategy merge
```

#### **Performance issues**
```bash
# Check repository size
du -sh .git/

# Optimize Git repository
git gc --aggressive
git prune

# Adjust cache settings
cc config set performance.cache_size 500
```

### **Debug Mode**
```bash
# Enable debug logging
export CASCADE_LOG_LEVEL=debug
cc stacks push

# Check logs
tail -f ~/.cascade/logs/cascade.log
```

### **Getting Help**
```bash
# Built-in help
cc --help
cc stack --help
cc stacks create --help

# System diagnostics
cc doctor --verbose

# Check configuration
cc config list
```

---

## 📞 **Support Resources**

- **[Installation Guide](./INSTALLATION.md)** - Setup and installation help
- **[Troubleshooting Guide](./TROUBLESHOOTING.md)** - Common issues and solutions
- **[Configuration Reference](./CONFIGURATION.md)** - Complete settings guide
- **[GitHub Issues](https://github.com/JAManfredi/cascade-cli/issues)** - Bug reports and feature requests
- **[Discussions](https://github.com/JAManfredi/cascade-cli/discussions)** - Community support

---

*For more detailed information on specific topics, see the linked documentation files.* 