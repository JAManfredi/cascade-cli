# Bitbucket Integration Guide

This guide covers Cascade CLI's integration with Bitbucket for pull request management and automated landing.

## Auto-Land Feature

The auto-land functionality allows you to automatically merge pull requests when they meet Bitbucket's server-side requirements, with intelligent polling and safety checks.

### Quick Start

```bash
# Simple auto-land all ready PRs
cc stack autoland

# Auto-land with custom settings
cc stack autoland --wait-for-builds --strategy merge

# Traditional syntax (equivalent to autoland)
cc stack land --all --auto
```

### Command Variants

#### `cc stack autoland` (Recommended)
**Shorthand for `--all --auto`** - automatically lands all ready PRs using Bitbucket's authoritative merge endpoint:

- ✅ **Server-side validation**: Uses Bitbucket's merge endpoint to check all requirements
- ✅ **Respects all rules**: Honors server-configured approval, build, and conflict requirements
- ✅ **Author filtering**: Optional allowlist for trusted authors only
- ✅ **Build waiting**: Waits for builds to complete before attempting merge

#### Traditional Flags

| Flag | Behavior | Use Case |
|------|----------|----------|
| `--auto` | Uses Bitbucket's authoritative merge endpoint | **All repositories** - respects server rules |
| `--wait-for-builds` | Only waits for builds, skips server merge validation | **Testing/development** - when you want to bypass some checks |

**For all production repositories**: Use `autoland` (default `--auto`) as it respects your Bitbucket server's actual merge requirements.

### Polling Behavior

When auto-landing with `--wait-for-builds`, the system polls every **30 seconds** to check build status:

```rust
// Poll every 30 seconds until builds complete or timeout
sleep(Duration::from_secs(30)).await;
```

**Default timeout**: 30 minutes (1800 seconds)

### Smart Merge Logic

Instead of duplicating Bitbucket's complex merge rules, Cascade asks Bitbucket directly:

1. **Query Bitbucket**: "Can this PR be merged right now?"
2. **Respect the answer**: Only attempt merge if Bitbucket says "yes"
3. **Display helpful status**: Show what's blocking merge (builds, approvals, conflicts)

This approach works with **any Bitbucket configuration**:
- Custom approval requirements (1, 2, 3+ approvals)
- Build requirements (any CI/CD system)
- Merge restrictions (branch permissions)
- Conflict detection

### Auto-Merge Conditions

```rust
// Simplified conditions - let Bitbucket do the validation
AutoMergeConditions {
    merge_strategy: MergeStrategy::Squash,  // How to merge
    wait_for_builds: true,                  // Wait for builds?
    build_timeout: Duration::from_secs(1800), // 30 min timeout
    allowed_authors: None,                  // Optional: restrict by author
}
```

#### Scenario 1: Development Team with Server Restrictions
Your Bitbucket server requires 2 approvals + 2 passing builds:

```bash
# Perfect - respects all server requirements automatically
cc stack autoland
```

#### Scenario 2: Personal Repository
Minimal server restrictions, you only care about builds:

```bash
# Skip server validation, just wait for builds
cc stack autoland --wait-for-builds
```

#### Scenario 3: Trusted Authors Only
Only auto-land PRs from specific team members:

```bash
# Configure in .cascade/config.json
{
  "auto_merge": {
    "allowed_authors": ["alice", "bob", "charlie"]
  }
}
```

### Usage Examples

#### Basic Stack Management
```bash
# 📊 Check if your stack needs updates (read-only)
cc stack check

# 🔄 Sync with remote changes (recommended daily workflow)
cc stack sync

# 🔄 Force sync even if there are issues
cc stack sync --force

# 🔄 Interactive sync for manual conflict resolution
cc stack sync --interactive

# 🔄 Sync without cleaning up merged branches
cc stack sync --skip-cleanup
```

#### Complete Development Workflow
```bash
# Morning routine: sync with latest changes
cc stack sync

# Make changes and push to stack
cc stack push --message "Add feature X"

# Submit for review
cc stack submit --all

# Later: sync again to get latest changes
cc stack sync

# Land completed PRs
cc stack land
```

#### Simplified Landing Commands ✅
```bash
# 🎯 Default: Land all ready PRs with safety checks (RECOMMENDED)
cc stack land

# 🎯 Land specific entry by number (1-based) 
cc stack land 2

# 🔍 Preview what would be landed
cc stack land --dry-run

# ⚠️ Force land ignoring safety checks (dangerous)
cc stack land --force

# 🔒 Use server-side validation (extra safety)
cc stack land --auto

# 🕐 Wait for builds before landing
cc stack land --wait-for-builds

# 🚀 Shorthand for land --auto
cc stack autoland
```

#### AutoLand Shorthand
```bash
# Equivalent to: cc stack land --auto
cc stack autoland
```

#### Advanced Options
```bash
# Use merge strategy instead of squash
cc stack autoland --strategy merge

# Custom build timeout (20 minutes)
cc stack autoland --build-timeout 1200

# Wait for builds but skip other validation
cc stack autoland --wait-for-builds
```

#### Status and Monitoring
```bash
# Check stack status with merge readiness
cc stack show --mergeable

# List all PRs with their status
cc stack prs

# Get detailed status for debugging
cc stack status
```

### Error Handling

Auto-land provides clear feedback when PRs can't be merged:

```
❌ PR #123: Cannot auto-merge
   - Build failed: CI/CD pipeline
   - Missing 1 approval from: [@reviewer1]
   - Conflicts in: src/main.rs

⏳ PR #124: Waiting for builds
   - CI/CD pipeline: In Progress (5 minutes remaining)
   - Ready to merge once builds pass

✅ PR #125: Merged successfully
   - Strategy: squash
   - Commit: abc123 "Implement feature X"
```

### Troubleshooting

#### Auto-land not working?

1. **Check PR status**: `cc stack show --mergeable`
2. **Verify Bitbucket rules**: Does the PR meet server requirements?
3. **Review build status**: Are builds passing?
4. **Check conflicts**: Are there merge conflicts?

#### Common issues:

- **"Not mergeable"**: Check approvals, builds, and conflicts in Bitbucket UI
- **Build timeout**: Increase timeout with `--build-timeout 3600`
- **Author restricted**: Ensure PR author is in `allowed_authors` list

## API Integration Details

### Authentication
Configure Bitbucket authentication:

```bash
cc config set bitbucket.url "https://bitbucket.company.com"
cc config set bitbucket.username "your-username"

# Store token securely (recommended)
cc config set bitbucket.token "your-app-password"
```

### Supported Endpoints

Cascade integrates with these Bitbucket Server APIs:

- **Merge endpoint**: `GET/POST /rest/api/1.0/projects/{project}/repos/{repo}/pull-requests/{id}/merge`
- **Build status**: `GET /rest/build-status/1.0/commits/{commit}`
- **PR details**: `GET /rest/api/1.0/projects/{project}/repos/{repo}/pull-requests/{id}`
- **Participants**: `GET /rest/api/1.0/projects/{project}/repos/{repo}/pull-requests/{id}/participants`

### Rate Limiting

Cascade respects Bitbucket API limits:
- **Polling frequency**: 30 seconds (configurable)
- **Concurrent requests**: Limited to avoid overwhelming server
- **Exponential backoff**: On rate limit errors

## Security Considerations

### Token Permissions
App passwords need these permissions:
- **Repositories**: Read, Write
- **Pull requests**: Read, Write
- **Account**: Read (for user info)

### Author Allowlist
For security, restrict auto-merge to trusted authors:

```json
{
  "auto_merge": {
    "allowed_authors": ["team-lead", "senior-dev", "ci-bot"]
  }
}
```

This prevents unauthorized auto-merging from:
- External contributors
- Compromised accounts
- Untrusted automation

### Audit Trail
All auto-merge activities are logged:
- **Who**: PR author and merger
- **When**: Timestamp of merge
- **What**: Commit hash and strategy
- **Why**: Merge trigger (manual/auto)

### Best Practices

1. **Use app passwords**: More secure than user passwords
2. **Restrict authors**: Don't allow auto-merge from everyone
3. **Monitor activity**: Review auto-merge logs regularly
4. **Set reasonable timeouts**: Don't wait forever for builds
5. **Test in dev**: Validate auto-land in development repos first

## Stack Landing Mechanics

### Current Implementation ✅
When landing stacked PRs, each PR is merged individually in dependency order **with automatic retargeting**:

```
PR #101: feature-base → main        (merges first)
PR #102: feature-part2 → feature-base   (merges second)  
PR #103: feature-final → feature-part2   (merges third)
```

### Automatic Retargeting System 🔄
**NEW**: After each PR merge, the system automatically retargets remaining PRs to the latest base:

1. **Merge base PR**: Land PR #101 (`feature-base` → `main`) ✅
2. **Auto-retarget dependents**: Change PR #102 target from `feature-base` to `main` 🔄 **AUTOMATIC**
3. **Update Bitbucket PRs**: Use force-push to preserve PR history and comments 🔄 **AUTOMATIC**
4. **Continue sequence**: Process remaining PRs with updated targets 🔄 **AUTOMATIC**

### How It Works

The landing system now integrates with the sophisticated rebase engine:

- **After each merge**: Triggers rebase system to retarget remaining PRs
- **Branch mapping**: Creates old→new branch mapping for PR updates  
- **PR preservation**: Uses `force_push_branch` to update PRs without losing history
- **Conflict resolution**: Leverages existing conflict resolution for complex cases
- **Automatic comments**: Adds explanatory comments to updated PRs

### Landing Commands

All these commands now include automatic retargeting:

```bash
# 🎯 Default: Land all ready PRs with safety checks (RECOMMENDED)
cc stack land

# 🎯 Land specific entry by number (1-based) 
cc stack land 2

# 🔍 Preview what would be landed
cc stack land --dry-run

# ⚠️ Force land ignoring safety checks (dangerous)
cc stack land --force

# 🔒 Use server-side validation (extra safety)
cc stack land --auto

# 🕐 Wait for builds before landing
cc stack land --wait-for-builds

# 🚀 Shorthand for land --auto
cc stack autoland
```

### Benefits

✅ **No manual intervention**: Never need `cc stack rebase --onto main` manually  
✅ **Preserves PR history**: All reviews, comments, and discussions remain intact  
✅ **Handles conflicts**: Uses intelligent conflict resolution system  
✅ **Progress transparency**: Shows retargeting progress and results  
✅ **Fallback guidance**: Provides manual steps if auto-retargeting fails

### Conflict Resolution Workflow 🚨

If conflicts occur during auto-retargeting, the land operation will pause and provide clear guidance:

```bash
cc stack land                     # Start landing all ready PRs

# ✅ PR #1 lands successfully
# 🔄 Auto-retargeting begins  
# ❌ Conflict detected during retargeting!

# CLI Output:
#   ❌ Auto-retargeting conflicts detected!
#   📝 To resolve conflicts and continue landing:
#      1. Resolve conflicts in the affected files
#      2. Stage resolved files: git add <files>
#      3. Continue the process: cc stack continue-land
#      4. Or abort the operation: cc stack abort-land
#   💡 Check current status: cc stack land-status
```

#### Manual Resolution Steps

```bash
# 1. Check what conflicts need resolution
cc stack land-status

# 2. Edit conflicted files manually
vim src/conflicted-file.rs       # Resolve <<<<<<< ======= >>>>>>> markers

# 3. Stage resolved files  
git add src/conflicted-file.rs

# 4. Continue the land operation
cc stack continue-land            # Resumes landing remaining PRs

# 5. Alternative: Abort if conflicts too complex
cc stack abort-land              # Restores pre-land state
```

### Base Branch Update Mechanism 📥

**NEW**: The land command now automatically updates the base branch after each PR merge:

1. **Merge PR #1** → `main` gets new commits ✅
2. **Update local base**: `git pull origin main` 🔄 **AUTOMATIC**  
3. **Retarget remaining PRs** to updated `main` 🔄 **AUTOMATIC**
4. **Continue with next PR** using latest base ✅

### ✅ **FIXED**: Industry-Standard Sync Commands

We now match the industry standard (Graphite) for sync functionality:

**NEW Behavior** (matches `gt sync`):
```bash
cc stack sync    # ✅ Pulls main + restacks + cleans up merged branches  
cc stack check   # ✅ Read-only status validation (old sync behavior)
```

This provides the intuitive workflow developers expect from modern stacked diff tools.

### Sync vs Check Commands ✨

#### **NEW**: Proper Sync (Industry Standard)
```bash
cc stack sync    # 🔄 Pull + rebase + cleanup (like Graphite's gt sync)
```

**What it does**:
1. **📥 Pull latest changes** from base branch (e.g., main)
2. **🔍 Check if rebase needed** based on new commits
3. **🔀 Auto-rebase stack** onto updated base if needed
4. **🔄 Update PRs** with preserved history using force-push
5. **🧹 Cleanup merged branches** (optional with `--skip-cleanup`)

**Options**:
- `--force`: Continue even if pull/checkout fails
- `--skip-cleanup`: Skip merged branch cleanup
- `--interactive`: Use interactive mode for conflict resolution

#### **Status Check Only**
```bash
cc stack check   # 📊 Read-only status check (old sync behavior)
```

**What it does**:
- ✅ Validates stack integrity
- ✅ Checks if base branch exists
- ✅ Detects if stack needs sync
- ✅ Updates stack status (`Clean`/`NeedsSync`/etc.)
- ❌ **Does NOT** pull or rebase
