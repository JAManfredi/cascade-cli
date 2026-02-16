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

## Core Concepts

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

## Command Reference

### **🎯 Core Commands**

#### **`ca init`** - Initialize Repository
Initialize Cascade CLI in a Git repository.

```bash
ca init [OPTIONS]

# Options:
--bitbucket-url <URL>     # Bitbucket Server URL
--project <PROJECT>       # Project key
--repository <REPO>       # Repository name
--force                   # Overwrite existing configuration
```

**Examples:**
```bash
# Interactive initialization
ca init

# Manual configuration
ca init --bitbucket-url https://bitbucket.company.com --project DEV --repository my-app

# Force reconfiguration
ca init --force
```

#### **`ca setup`** - Interactive Setup Wizard
Guided configuration for first-time users.

```bash
ca setup [OPTIONS]

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

#### **`ca stacks create`** - Create New Stack
Create a new stack for organizing related commits.

```bash
ca stacks create <NAME> [OPTIONS]

# Options:
--base <BRANCH>           # Base branch (default: current branch)
--description <DESC>      # Stack description
--activate               # Activate after creation (default: true)
```

**Examples:**
```bash
# Basic stack creation
ca stacks create feature-auth --base develop

# With description
ca stacks create fix-performance --base main --description "Database query optimizations"

# Create without activating
ca stacks create future-feature --base develop --no-activate
```

#### **`ca stacks list`** - List All Stacks
Display all stacks with their status and information.

```bash
ca stacks list [OPTIONS]

# Options:
--verbose, -v            # Show detailed information
--active                 # Show only active stack
--format <FORMAT>        # Output format (name, id, status)
```

**Examples:**
```bash
# Simple list
ca stacks list

# Detailed view
ca stacks list --verbose

# Only active stack
ca stacks list --active

# Custom format
ca stacks list --format status
```

#### **`ca stack`** - Display Stack Details
Show detailed information about a specific stack.

```bash
ca stack [NAME]

# Arguments:
[NAME]                   # Stack name (defaults to active stack)
```

**Output includes:**
- Stack metadata (name, description, base branch)
- All stack entries with commit details
- Pull request status and links
- Dependency information

#### **`ca switch`** - Activate Stack
Switch to a different stack, making it the active stack.

```bash
ca switch <NAME>

# Arguments:
<NAME>                   # Stack name to activate
```

**Examples:**
```bash
ca switch feature-auth
ca switch fix-bugs
```

#### **`ca stacks delete`** - Remove Stack
Delete a stack and optionally its associated branches.

```bash
ca stacks delete <NAME> [OPTIONS]

# Options:
--force                  # Skip confirmation prompt
--keep-branches         # Keep associated branches
```

**Examples:**
```bash
# With confirmation
ca stacks delete old-feature

# Force deletion
ca stacks delete temp-stack --force

# Delete but keep branches
ca stacks delete feature-x --keep-branches
```

### **🎯 Entry Editing (Modern Convenience)**

Cascade CLI provides modern convenience commands for editing specific stack entries without manual Git operations.

#### **`ca entry checkout`** - Interactive Entry Checkout
Checkout a specific stack entry for editing with intelligent tracking.

```bash
ca entry checkout [ENTRY] [OPTIONS]

# Arguments:
[ENTRY]                  # Entry number (1-based index)

# Options:
--direct                 # Skip interactive picker when using entry number
--yes, -y               # Skip confirmation prompts
```

**Examples:**
```bash
# Interactive picker (recommended)
ca entry checkout
# Shows TUI interface to select which entry to edit

# Direct checkout
ca entry checkout 1     # Checkout first entry
ca entry checkout 3 --yes  # Skip confirmation

# Direct mode (for scripting)
ca entry checkout 2 --direct --yes
```

**What it does:**
- ✅ Enters edit mode tracking for safety
- ✅ Checks out the specific commit safely
- ✅ Preserves stack state and metadata
- ✅ Shows clear guidance for next steps

#### **`ca entry status`** - Edit Mode Status
Show current edit mode status and guidance.

```bash
ca entry status [OPTIONS]

# Options:
--quiet                  # Brief status output (for scripts)
```

**Examples:**
```bash
# Detailed status
ca entry status

# Brief output
ca entry status --quiet
# Output: "active:entry-uuid" or "inactive"
```

#### **`ca entry list`** - List Entries with Edit Status
List all entries in the active stack with edit status indicators.

```bash
ca entry list [OPTIONS]

# Options:
--verbose, -v           # Show detailed information
```

**Examples:**
```bash
# Basic list with edit indicators
ca entry list

# Detailed view
ca entry list --verbose
```

**🎯 Modern Entry Editing Workflow:**
```bash
# 1. Select entry to edit
ca entry checkout         # Interactive picker

# 2. Make changes normally
# (changes are auto-staged)

# 3. Amend the entry (automatic restacking!)
ca entry amend -m "Add database schema (fixed column types)"

# 4. Verify changes
ca entry list           # Check updated status
ca stack               # See full stack state
```

**💡 Benefits over Manual Git:**
- **Safety**: Tracks edit state, prevents corruption
- **Convenience**: No need to remember commit hashes
- **Intelligence**: Interactive picker with rich information
- **Guidance**: Clear next steps and status tracking
- **Automatic**: Dependent entries are rebased automatically

---

#### **`ca entry amend`** - Amend Current Entry with Automatic Restacking

Amend the current stack entry's commit and automatically rebase all dependent entries onto the new commit.

**Synopsis:**
```bash
ca entry amend [OPTIONS]
```

**Options:**
- `-m, --message <MESSAGE>` - New commit message (optional, uses editor if not provided)
- `--push` - Automatically force-push after amending (if PR exists)

**How It Works:**
1. Automatically stages all modified tracked files (like `git commit -a --amend`)
2. Amends the current entry's commit
3. **Automatically rebases** all dependent entries onto the amended commit
4. Updates working branch to top of stack
5. Updates stack metadata

**Examples:**
```bash
# Amend with new message
ca entry amend -m "Fixed validation logic"

# Amend and open editor for message
ca entry amend

# Amend and push to PR
ca entry amend --push

# Just amend (keeps same message)
ca entry amend
```

**Important Notes:**
- ✅ **Automatic restacking**: No need to run `ca sync` - dependent entries are updated automatically
- ✅ **Auto-staging**: All modified tracked files are included (no need for `git add`)
- ✅ **Safety**: If conflicts occur, you'll get clear recovery instructions
- ⚠️ **Must be on stack entry**: Use `ca entry checkout <N>` first

**Conflict Resolution:**
If dependent entries have conflicts during automatic restacking:
```bash
# Cascade pauses and shows:
# "Failed to restack entry #4: conflicts"

# 1. Resolve conflicts in your editor
# 2. Continue the restack
ca entry continue

# Or abort and undo changes
ca entry abort
```

---

#### **`ca entry continue`** - Continue After Resolving Conflicts

Continue an in-progress restack after manually resolving conflicts from `ca entry amend`.

**Synopsis:**
```bash
ca entry continue
```

**When to Use:**
- After `ca entry amend` hits conflicts during automatic restacking
- After resolving all conflict markers in your editor

**What It Does:**
1. Auto-stages resolved conflict files
2. Completes the cherry-pick (bypassing hooks)
3. Updates entry branch pointer to new commit
4. Updates stack metadata
5. Cleans up temporary branches
6. Leaves you on the resolved entry branch

**Example Workflow:**
```bash
# Amend entry #3
ca entry checkout 3
ca entry amend -m "Updated schema"

# Conflict on entry #4!
# Error: Failed to restack entry #4: conflicts

# Resolve conflicts
vim src/models.rs  # Fix conflict markers
git status         # Check what needs resolving

# Continue
ca entry continue

# Complete the stack
ca sync
```

**Next Steps After Continue:**
- Run `ca sync` to finish rebasing remaining entries
- Run `ca validate` to verify stack consistency

---

#### **`ca entry abort`** - Abort In-Progress Restack

Abort an in-progress restack and undo partial changes from `ca entry amend`.

**Synopsis:**
```bash
ca entry abort
```

**When to Use:**
- After `ca entry amend` hits conflicts you can't resolve
- When you want to undo a failed restack attempt
- To get back to a clean state

**What It Does:**
1. Aborts the cherry-pick (bypassing hooks)
2. Cleans up temporary branches  
3. Returns you to a clean Git state
4. Stack may be partially inconsistent

**Example:**
```bash
# Amend hits conflicts
ca entry amend -m "Major refactor"
# Error: Failed to restack entry #4: conflicts

# Decide to abort instead of resolving
ca entry abort

# Check and fix stack state
ca validate

# Choose "Reset" or "Incorporate" as needed
```

**After Aborting:**
1. Run `ca validate` to check stack state
2. Fix any inconsistencies (usually choose "Reset")
3. Try a different approach or smaller changes

---

### **📤 Stack Operations**

#### **`ca push`** - Add Commits to Stack
Add commits to the active stack. By default, pushes all unpushed commits.

```bash
ca push [OPTIONS]

# Options:
--branch <NAME>         # Custom branch name for this commit
--message <MSG>         # Commit message (if creating new commit)
--commit <HASH>         # Use specific commit instead of HEAD
--since <REF>           # Push commits since reference (e.g., HEAD~3)
--commits <HASHES>      # Push specific commits (comma-separated)
--squash <N>            # 🎉 Squash last N commits into 1 clean commit
--squash-since <REF>    # 🎉 Squash all commits since reference
--yes, -y               # Skip confirmation prompts
--dry-run               # Preview commits without pushing
```

**Stale Base Detection:** When the base branch has moved forward since your branch diverged, `ca push` warns you and suggests rebasing first. Use `--yes` to skip this check.

**Commit Confirmation:** Before pushing, `ca push` shows a numbered list of commits with authors. Commits from other authors are highlighted. The default confirmation is `yes` for same-author commits and `no` for mixed-author commits. Use `--yes` to skip confirmation.

**Default Behavior:** When no specific targeting options are provided, `ca push` pushes **all unpushed commits** since the last stack push.

**Squash Workflow Examples:**
```bash
# Make incremental commits during development
git commit -m "WIP: start feature"
git commit -m "WIP: add core logic"
git commit -m "WIP: fix bugs"
git commit -m "Final: complete feature with tests"

# 🔍 See unpushed commits and get squash suggestions
ca stack
# 🚧 Unpushed commits (4): use 'ca stacks push --squash 4' to squash them
#    1. WIP: start feature (abc123)
#    2. WIP: add core logic (def456)
#    3. WIP: fix bugs (ghi789)
#    4. Final: complete feature with tests (jkl012)
# 💡 Squash options:
#    ca stacks push --squash 4           # Squash all unpushed commits
#    ca stacks push --squash 3           # Squash last 3 commits only

# 🎉 Smart squash automatically detects "Final:" commits and creates intelligent messages
ca stacks push --squash 4
# ✅ Smart message: Complete feature with tests (automatically extracted from "Final:" commit)

# Alternative patterns that smart squash recognizes:
git commit -m "WIP: authentication work"
git commit -m "Add user authentication with OAuth"  # Uses this descriptive message
ca stacks push --squash 2  # Result: "Add user authentication with OAuth"

git commit -m "fix typo"
git commit -m "fix bug"  
git commit -m "refactor cleanup"
ca stacks push --squash 3  # Result: "Refactor cleanup" (uses last commit)
```

**Branch Naming:** Generated from final squashed commit message using Cascade CLI's branch naming rules.

**Examples:**
```bash
# Push all unpushed commits (default behavior)
git commit -m "Add user authentication"
git commit -m "Add password validation"
ca stacks push  # Pushes both commits as separate stack entries

# Push specific commit only
ca stacks push --commit abc123

# Push commits since specific reference
ca stacks push --since HEAD~3

# Push specific commits
ca stacks push --commits abc123,def456,ghi789

# Push with custom branch name
ca stacks push --branch custom-auth-branch

# Squash multiple commits before pushing
ca stacks push --squash 3  # Squashes last 3 commits into one

# Squash commits since reference
ca stacks push --squash-since HEAD~5
```

#### **`ca stacks pop`** - Remove Entry from Stack
Remove the top entry from the stack.

```bash
ca stacks pop [OPTIONS]

# Options:
--keep-branch           # Keep the associated branch
--force                 # Skip confirmation
```

**Examples:**
```bash
# Remove top entry
ca pop

# Keep the branch
ca pop --keep-branch

# Force removal
ca pop --force
```

#### **`ca drop`** - Remove Entries by Position
Remove one or more stack entries by position. Unlike `ca pop` which only removes the top entry, `ca drop` can remove any entry and supports ranges.

```bash
ca drop <ENTRY> [OPTIONS]

# Arguments:
<ENTRY>                 # Position or range (e.g., "3", "1-5", "1,3,5")

# Options:
--keep-branch           # Keep the associated branch(es)
--keep-pr               # Keep the PR open on Bitbucket (don't decline it)
--force, -f             # Skip all prompts (declines PRs, deletes branches)
--yes, -y               # Skip entry confirmation prompt
```

**Behavior:**
- Removes entries and reparents any children to the removed entry's parent
- Refuses to drop merged entries (use `ca stacks cleanup` instead)
- Deletes associated branches unless `--keep-branch` is specified
- Declines associated Bitbucket PRs unless `--keep-pr` is specified
- `--force` does everything without prompting; combine with `--keep-pr` or `--keep-branch` to protect specific resources

**Examples:**
```bash
# Remove a single entry
ca drop 3

# Remove a range of entries
ca drop 1-5

# Remove specific entries
ca drop 1,3,5

# Remove entry but keep its branch
ca drop 3 --keep-branch

# Remove entry but leave PR open
ca drop 3 --keep-pr

# Skip all prompts (declines PRs and deletes branches)
ca drop 3 --force

# Skip prompts but keep PRs open
ca drop 3 --force --keep-pr
```

#### **`ca submit`** - Create Pull Requests
Submit stack entries as pull requests. By default, submits all unsubmitted entries.

```bash
ca submit [ENTRY] [OPTIONS]

# Arguments:
[ENTRY]                 # Entry index (defaults to all unsubmitted entries)

# Options:
--title <TITLE>         # PR title override
--description <DESC>    # PR description
--range <RANGE>         # Submit range of entries (e.g., "1-3" or "2,4,6")
--no-draft              # Create as ready PR (default is draft)
--no-open               # Don't open PR in browser (default opens)
--reviewers <USERS>     # Comma-separated reviewer list
```

**Default Behavior:** When no specific entry is provided, `ca submit` submits **all unsubmitted entries** as separate pull requests.

**Examples:**
```bash
# Submit all unsubmitted entries (default behavior)
ca submit

# Submit specific entry
ca submit 2

# Submit range of entries
ca submit --range 1-3

# Submit specific entries 
ca submit --range 2,4,6

# Submit with custom details
ca submit --title "Add OAuth integration" --description "Implements Google OAuth2 flow"

# Create ready (non-draft) PRs
ca submit --no-draft

# Submit without opening browser
ca submit --no-open

# Add reviewers
ca submit --reviewers "alice,bob,charlie"
```

#### **`ca sync`** - Synchronize with Remote
Update stack with latest changes from base branch and dependencies.

```bash
ca sync [OPTIONS]

# Options:
--force                 # Force sync even with conflicts
--interactive           # Interactive mode for conflict resolution
--cleanup               # Also cleanup merged branches after sync
```

**Examples:**
```bash
# Standard sync (uses force-push strategy to preserve PR history)
ca sync

# Force sync with conflicts
ca sync --force

# Interactive mode for manual conflict resolution
ca sync --interactive

# Sync and cleanup merged branches
ca sync --cleanup
```

**Conflict Resolution:**
If `ca sync` encounters conflicts it cannot auto-resolve:
```bash
# After ca sync reports conflicts:
# 1. Resolve conflicts manually
git add <resolved-files>

# 2. Continue the sync
ca sync continue

# OR abort the sync
ca sync abort
```

#### **`ca sync continue`** - Continue After Resolving Conflicts
Continue an in-progress sync after manually resolving conflicts.

```bash
ca sync continue
```

**When to use:**
- After `ca sync` hits conflicts during rebase
- After you've resolved conflicts and staged changes with `git add`

**What it does:**
1. Completes the current cherry-pick
2. Updates stack metadata
3. Re-enters the sync loop to process remaining entries
4. Cleans up temporary branches
5. Returns you to your original branch

**Example:**
```bash
ca sync                    # Hits conflict on entry #2

# Resolve conflicts...
vim conflict.txt
git add conflict.txt

ca sync continue          # Continues with entries #3, #4, #5...
```

#### **`ca sync abort`** - Abort In-Progress Sync
Abort an in-progress sync and clean up temporary state.

```bash
ca sync abort
```

**When to use:**
- After `ca sync` hits conflicts you can't resolve
- When you want to start over with a fresh sync
- To recover from a stuck sync state

**What it does:**
1. Aborts the current cherry-pick
2. Cleans up all temporary branches
3. Returns you to your original branch
4. Deletes sync state file

**Example:**
```bash
ca sync                    # Hits complex conflicts

# Decide to abort and try a different approach
ca sync abort

# Now you're back to clean state
ca sync --interactive      # Try with interactive mode
```

#### **`ca rebase`** - Rebase Stack
Rebase all stack entries on latest base branch using smart force push strategy (industry standard).

```bash
ca rebase [OPTIONS]

# Options:
--interactive          # Interactive rebase mode for manual conflict resolution
--onto <branch>        # Rebase onto specific branch (defaults to stack's base)
--strategy <strategy>  # Rebase strategy: force-push (default) or interactive
```

**Smart Force Push Behavior:**
When rebasing, Cascade CLI uses the industry-standard approach:
1. Creates temporary branches for rebasing (`feature-temp-123456`)
2. Cherry-picks commits onto the new base
3. Force-pushes temp content to original branches (`feature`)
4. **Preserves ALL existing PRs** and review history
5. Cleans up temporary branches automatically

This approach follows industry standards (Graphite, Phabricator, spr, GitHub CLI) and ensures reviewers never lose context, comments, or approval history. Branch names stay the same, so PRs remain intact.

**Examples:**
```bash
# Standard rebase with PR history preservation
ca rebase

# Interactive rebase
ca rebase --interactive

# Rebase onto specific branch
ca rebase --onto develop

# Using stacks subcommand (equivalent)
ca stacks rebase
ca stacks rebase --interactive
```

**Conflict Resolution:**
If rebase encounters conflicts:
```bash
# After ca rebase reports conflicts:
# 1. Resolve conflicts manually
git add <resolved-files>

# 2. Continue the rebase
ca rebase continue

# OR abort the rebase
ca rebase abort
```

**What you'll see:**
```bash
$ ca stacks rebase

🔄 Rebasing stack: authentication
   📋 Rebasing 2 entries using force-push strategy
   
   🔄 Processing commits:
      ✅ Force-pushed add-auth-temp content to add-auth (preserves PR #123)
      ✅ Force-pushed add-tests-temp content to add-tests (preserves PR #124)
   
   🧹 Cleaned up 2 temporary branches

   ✅ 2 commits successfully rebased - PR history preserved
```

### **📊 Status and Information**

#### **`ca repo`** - Show Repository Overview
Display comprehensive status of current repository and stacks.

```bash
ca repo [OPTIONS]

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

#### **`ca stacks status`** - Stack-Specific Status
Show detailed status for current or specified stack.

```bash
ca stacks status [NAME]

# Arguments:
[NAME]                  # Stack name (defaults to active stack)
```

#### **`ca stacks prs`** - List Pull Requests
Show all pull requests associated with stacks.

```bash
ca stacks prs [OPTIONS]

# Options:
--stack <NAME>          # Filter by stack name
--status <STATUS>       # Filter by PR status (open, merged, declined)
--format <FORMAT>       # Output format (table, json)
```

**Examples:**
```bash
# All PRs
ca stacks prs

# PRs for specific stack
ca stacks prs --stack feature-auth

# Only open PRs
ca stacks prs --status open
```

### **🎨 Visualization**

#### **`ca viz stack`** - Stack Diagram
Generate visual representation of a stack.

```bash
ca viz stack [NAME] [OPTIONS]

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
ca viz stack

# Mermaid diagram
ca viz stack --format mermaid

# Save to file
ca viz stack --format dot --output stack.dot

# Compact mode
ca viz stack --compact
```

#### **`ca viz deps`** - Dependency Graph
Show dependencies between all stacks.

```bash
ca viz deps [OPTIONS]

# Options:
--format <FORMAT>       # Output format (ascii, mermaid, dot, plantuml)
--output <FILE>         # Save to file
--compact              # Compact display mode
--no-colors            # Disable colored output
```

**Examples:**
```bash
# ASCII dependency graph
ca viz deps

# Export to Mermaid
ca viz deps --format mermaid --output deps.md

# Graphviz format for advanced visualization
ca viz deps --format dot --output deps.dot
```

### **🖥️ Interactive Tools**

#### **`ca tui`** - Terminal User Interface
Launch interactive stack browser.

```bash
ca tui
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

#### **`ca hooks install`** - Install All Hooks
Install all Cascade Git hooks for workflow automation.

```bash
ca hooks install [OPTIONS]

# Options:
--force                 # Overwrite existing hooks
```

#### **`ca hooks uninstall`** - Remove All Hooks
Remove all Cascade Git hooks.

```bash
ca hooks uninstall
```

#### **`ca hooks status`** - Show Hook Status
Display installation status of all Git hooks.

```bash
ca hooks status
```

#### **`ca hooks add`** - Install Specific Hook
Install a specific Git hook.

```bash
ca hooks add <HOOK>

# Hook types:
post-commit            # Auto-add commits to active stack
pre-push              # Prevent dangerous pushes to protected branches
commit-msg            # Validate commit message format
prepare-commit-msg    # Add stack context to commit messages
```

#### **`ca hooks remove`** - Remove Specific Hook
Remove a specific Git hook.

```bash
ca hooks remove <HOOK>
```

### **⚙️ Configuration**

#### **`ca config`** - Configuration Management
Manage Cascade CLI configuration settings.

```bash
ca config <SUBCOMMAND>

# Subcommands:
list                   # Show all configuration
get <KEY>             # Get specific value
set <KEY> <VALUE>     # Set configuration value
unset <KEY>           # Remove configuration value
```

**Examples:**
```bash
# List all configuration
ca config list

# Get specific setting
ca config get bitbucket.url

# Set configuration
ca config set bitbucket.token "your-token-here"

# Remove setting
ca config unset bitbucket.project
```

### **🔧 Utility Commands**

#### **`ca doctor`** - System Diagnostics
Run comprehensive system health check.

```bash
ca doctor [OPTIONS]

# Options:
--verbose, -v           # Show detailed diagnostics
--fix                  # Attempt to fix common issues
```

#### **`ca completions`** - Shell Completions
Manage shell completion installation.

```bash
ca completions <SUBCOMMAND>

# Subcommands:
install               # Auto-install for detected shells
status               # Show installation status
generate <SHELL>     # Generate completions for specific shell
```

#### **`ca version`** - Version Information
Display version and build information.

```bash
ca version [OPTIONS]

# Options:
--verbose, -v         # Show detailed build information
```

#### **`ca cleanup`** - Clean Up Temporary Branches
Remove orphaned temporary branches created during rebase operations.

```bash
ca cleanup [OPTIONS]

# Options:
--execute            # Actually delete branches (default is dry-run)
--force              # Force deletion even if branches have unmerged commits

# Examples:
ca cleanup                    # Dry-run: show what would be deleted
ca cleanup --execute          # Actually delete temp branches
ca cleanup --execute --force  # Force delete including unmerged branches
```

**When to use**: If a rebase operation is interrupted or fails, temporary branches 
with names like `feature-temp-1234567890` may be left behind. This command helps 
identify and remove them.

---

## Workflow Patterns

### **Feature Development Workflow**

#### **1. Start New Feature**
```bash
# Create feature stack
ca stacks create feature-user-profiles --base develop --description "User profile management system"

# Start development
git checkout develop
git pull origin develop
```

#### **2. Incremental Development**
```bash
# First increment: basic profile model
git add . && git commit -m "Add user profile model"
ca push

# Second increment: profile endpoints
git add . && git commit -m "Add profile CRUD endpoints"
ca push

# Third increment: profile validation
git add . && git commit -m "Add profile data validation"
ca push
```

#### **3. Submit for Review**
```bash
# Submit each increment as separate PRs
ca submit 1  # Submit profile model
ca submit 2  # Submit endpoints (depends on model)
ca submit 3  # Submit validation (depends on endpoints)
```

#### **4. Handle Review Feedback**
```bash
# Make changes to address feedback
git add . && git commit -m "Address review feedback: improve validation"

# Update existing PR
ca submit 3 --title "Updated: Add profile data validation"

# Or sync if dependencies changed
ca sync
```

#### **5. Merge and Clean Up**
```bash
# After PRs are approved and merged
ca pop  # Remove merged entries
ca pop
ca pop

# Or delete completed stack
ca stacks delete feature-user-profiles
```

### **Bug Fix Workflow**

#### **Quick Fix**
```bash
# Create fix stack
ca stacks create fix-login-bug --base main --description "Fix login timeout issue"

# Make fix
git add . && git commit -m "Fix login timeout in OAuth flow"
ca stacks push

# Submit immediately
ca stacks submit --reviewers "security-team"
```

#### **Complex Fix with Investigation**
```bash
# Investigation stack
ca stacks create investigate-memory-leak --base develop

# Add investigation commits
git commit -m "Add memory profiling tools"
ca stacks push

git commit -m "Identify memory leak in cache layer"
ca stacks push

git commit -m "Fix memory leak and add tests"
ca stacks push

# Submit investigation and fix separately
ca stacks submit 1 --title "Add memory profiling tools"
ca stacks submit 3 --title "Fix memory leak in cache layer"
```

---

## Advanced Usage

### **Custom Workflow Integration**

#### **CI/CD Integration**
```bash
# In CI pipeline
ca doctor --verbose           # Validate environment
ca stacks status --format json # Get status for reporting
ca viz deps --format dot      # Generate dependency graphs
```

#### **Pre-commit Hook Integration**
```bash
# Install hooks for automatic workflow
ca hooks install

# Hooks will automatically:
# - Add commits to active stack
# - Validate commit messages
# - Prevent dangerous operations
```

### **Large Repository Optimization**

#### **Performance Configuration**
```bash
# Optimize for large repos
ca config set performance.cache_size 2000
ca config set performance.parallel_operations true
ca config set network.timeout 120
```

#### **Selective Stack Management**
```bash
# Work with specific stacks only
ca stacks list --format name | grep feature- | xargs -I {} ca stacks validate {}
```

### **Advanced Visualization**

#### **Documentation Generation**
```bash
# Generate project architecture docs
ca viz deps --format mermaid --output docs/architecture.md

# Include in markdown
echo "# Project Architecture" > docs/full-arch.md
echo "## Stack Dependencies" >> docs/full-arch.md
ca viz deps --format mermaid >> docs/full-arch.md
```

#### **Custom Formats for Tools**
```bash
# Export for external tools
ca viz stack --format dot | dot -Tpng > stack-diagram.png
ca viz deps --format plantuml | plantuml -pipe > deps.svg
```

---

## Configuration

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
auto_cleanup_merged = true
prefer_rebase = true

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

## Troubleshooting

### **Common Issues and Solutions**

#### **"Stack not found" errors**
```bash
# List all stacks to verify names
ca stacks list

# Check if in correct repository
ca repo

# Re-initialize if needed
ca init --force
```

#### **Bitbucket connection issues**
```bash
# Test connection
ca doctor

# Verify token permissions
ca config get bitbucket.token

# Reconfigure if needed
ca setup --force
```

#### **Sync conflicts**
```bash
# Check conflict status
ca stacks status

# Resolve manually and continue
git add .
ca stacks rebase --continue

# Or abort and try different strategy
ca stacks rebase --abort
ca stacks sync --strategy merge
```

#### **Performance issues**
```bash
# Check repository size
du -sh .git/

# Optimize Git repository
git gc --aggressive
git prune

# Adjust cache settings
ca config set performance.cache_size 500
```

### **Debug Mode**
```bash
# Enable debug logging
export CASCADE_LOG_LEVEL=debug
ca stacks push

# Check logs
tail -f ~/.cascade/logs/cascade.log
```

### **Getting Help**
```bash
# Built-in help
ca --help
ca stack --help
ca stacks create --help

# System diagnostics
ca doctor --verbose

# Check configuration
ca config list
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