# 🔄 **Cascade CLI Workflows**

A comprehensive guide to real-world development workflows using Cascade CLI's stacked diff approach.

## 📋 **Table of Contents**

- [🌲 Understanding Git Branches vs Stacks](#understanding-git-branches-vs-stacks)
- [🚀 Modern Quick Workflows](#modern-quick-workflows)
  - [Fast Feature Development](#fast-feature-development)
  - [WIP to Clean Commits](#wip-to-clean-commits)
  - [Auto-Landing Ready PRs](#auto-landing-ready-prs)
- [🔄 Complex Scenarios](#complex-scenarios)

  - [Base Branch Updates (Smart Force Push)](#base-branch-updates-smart-force-push)
      - [Modifying Any Commit in Stack](#modifying-any-commit-in-stack)
  - [Managing Multiple Related Stacks](#managing-multiple-related-stacks)
  - [Handling Merge Conflicts During Rebase](#handling-merge-conflicts-during-rebase)
  - [Emergency Hotfix with Parallel Development](#emergency-hotfix-with-parallel-development)
  - [Stack Cleanup After Merges](#stack-cleanup-after-merges)
- [🚧 Future: Team Collaboration](#future-team-collaboration)

---

## 🌲 **Understanding Git Branches vs Stacks**

**The #1 source of confusion**: Git branches and Cascade stacks are **separate concepts** that work together.

### **🔍 The Key Insight**

```bash
# Git branch = Where you are
# Cascade stack = What you're working on
```

**Your current Git branch does NOT determine your active stack.**

### **📚 How It Works**

#### **1. Stacks Are Independent of Your Git Branch**
```bash
# Create a stack while on main
git checkout main
ca stacks create my-feature --base main

# Switch to a different Git branch
git checkout some-other-branch

# Your stack is STILL active!
ca stack  # Still shows "my-feature" stack
```

#### **2. Each Stack Has Its Own Base Branch**
```bash
# Different stacks can have different base branches
ca stacks create frontend-work --base main
ca stacks create hotfix --base release-1.2
ca stacks create experiment --base develop
```

#### **3. Stack Entries Create Individual Branches**
```bash
# When you push to a stack, Cascade creates branches automatically
ca push  # Creates feature/my-feature-1, feature/my-feature-2, etc.

# These branches are SEPARATE from your current working branch
git branch  # Shows: main, feature/my-feature-1, feature/my-feature-2
```

### **🔄 Two Common Workflow Patterns**

#### **Pattern 1: Work Directly on Base Branch (Recommended)**
```bash
git checkout main           # Start on main
ca stacks create my-feature  # Create stack based on main
# Make commits directly on main
ca push                     # Each commit becomes a stack entry with its own branch
```

#### **Pattern 2: Work on Feature Branch, Then Stack**
```bash
git checkout -b feature-branch
# Make several commits
ca stacks create my-feature --base main
ca push               # Add existing commits to stack
```

### **🤔 What Happens When You Switch Git Branches?**

**Current Behavior (Cascade CLI)**:
- Your **active stack remains the same**
- Stack state is **persisted in `.cascade/metadata.json`**
- You can work on your stack from **any Git branch**

```bash
# Start with stack active
ca stack  # Shows "my-feature" stack

# Switch Git branches
git checkout develop
git checkout -b random-experiment

# Stack is STILL active
ca stack  # Still shows "my-feature" stack
ca push  # Still adds to "my-feature" stack
```

### **🆚 How Other Tools Handle This**

#### **Graphite (`gt`)**
- **Branch-centric**: Each stack entry must be on its own branch
- **Auto-switches**: Changing Git branches can change your active stack context
- **Branch navigation**: `gt up`/`gt down` moves between branches in the stack

#### **Spr**
- **Commit-centric**: Focuses on commit relationships over branches
- **Single active**: One active "stack" of commits at a time
- **Branch-independent**: Stack state persists across branch switches (similar to Cascade)

#### **Sapling/Meta's Internal Tools**
- **Virtual branches**: Stacks are virtual concepts over commits
- **Branch abstraction**: Git branches are mostly hidden from users
- **Context switching**: Explicit commands to switch stack context

### **🚨 Potential Confusion Points**

#### **1. "Orphaned" Stack Feeling**
```bash
git checkout some-random-branch
ca stack  # Shows stack that doesn't match your current branch
```
**Solution**: Use `ca stacks switch` to explicitly change stacks.

#### **2. Commits from Wrong Branch**
```bash
git checkout feature-branch
# Commit some work
ca push  # Adds feature-branch commits to whatever stack is active
```
**Solution**: Always check `ca stack` before `ca push`.

#### **3. Multiple Stacks Confusion**
```bash
ca stacks list  # Shows multiple stacks
# Which one is active? Which branch am I on?
```
**Solution**: Use status indicators and explicit switching.

### **💡 Best Practices**

#### **1. Use Stack-Aware Status**
```bash
# Always check both Git and stack status
git status && ca stack
```

#### **2. Explicit Stack Switching**
```bash
# Don't rely on Git branch to determine stack
ca stacks switch my-other-feature
```

#### **3. Name Stacks Clearly**
```bash
# Use descriptive names that match your mental model
ca stacks create user-auth-backend --base main
ca stacks create mobile-ui-fixes --base develop
```

#### **4. One Stack Per Feature Branch (If Using Pattern 2)**
```bash
git checkout -b user-authentication
ca stacks create user-auth --base main
# Keep the feature branch and stack names aligned
```

### **🔧 Potential Improvements for Cascade CLI**

The current behavior could be enhanced with:

1. **Branch-aware default**: When creating a stack, use current branch name as default stack name
2. **Visual indicators**: Show current Git branch vs active stack in `ca stack`
3. **Auto-switch option**: Flag to auto-switch stacks when changing Git branches
4. **Stack-branch binding**: Option to bind a stack to a specific Git branch

## 🛡️ **Base Branch Protection (New!)**

**Cascade CLI now protects against accidentally polluting your base branch** with work-in-progress commits.

### **🚨 The Problem We Solve**

```bash
# ❌ DANGEROUS: Direct work on main
git checkout main
git commit -am "WIP: trying something"  # This commit is NOW ON MAIN!
ca push  # Oops, now main has WIP commits
```

### **✅ Built-in Safety Features**

#### **1. Auto-Detection & Warning**
```bash
git checkout main
ca push  # Cascade CLI detects base branch work

# Output:
# 🚨 WARNING: You're currently on the base branch 'main'
#    Making commits directly on the base branch is not recommended.
#    This can pollute the base branch with work-in-progress commits.
```

#### **2. Auto-Branch Creation**
```bash
# Let Cascade create a safe feature branch automatically
ca push --auto-branch

# Output:
# 🚀 Auto-creating feature branch 'feature/my-feature-work'...
# 🔄 Moving 2 commit(s) to new branch...
#    ✅ Moved a1b2c3d4
#    ✅ Moved e5f6g7h8
# ✅ Successfully moved 2 commit(s) to 'feature/my-feature-work'
```

#### **3. Source Branch Tracking**
Cascade CLI now tracks where each commit was originally made:

```bash
ca stack
# Output:
# 📚 Stack Entries:
#    1. a1b2c3d4 📝 Add authentication (from main)
#    2. e5f6g7h8 📝 Add validation (from feature/auth)
#    3. i9j0k1l2 📝 Add tests (from feature/auth)
```

#### **4. Manual Override (Not Recommended)**
```bash
# Force push from base branch (dangerous)
ca push --allow-base-branch
```

### **🔄 Updated Workflow Patterns**

#### **Pattern 1: Safe Feature Branch Workflow (Recommended)**
```bash
git checkout main
ca stacks create user-auth --base main

# Always work on feature branches
git checkout -b feature/authentication
git commit -am "Add login system"
git commit -am "Add password validation"

ca push  # Safe: commits are on feature branch
```

#### **Pattern 2: Auto-Branch Recovery**
```bash
# If you accidentally worked on main...
git checkout main
git commit -am "Oops, worked on main"

# Cascade CLI to the rescue:
ca push --auto-branch  # Safely moves commits to feature branch
```

#### **Pattern 3: Explicit Branch Creation**
```bash
git checkout main
ca push  # Cascade CLI warns and suggests options

# Follow the guidance:
git checkout -b feature/my-work
ca push  # Now safe
```

#### **Pattern 4: Emergency on Feature Branch**
If you happen to be on a feature branch that matches the commits you want to add:

```bash
git checkout feature/urgent-fix  
git commit -am "Fix critical bug"

# Safe because cascade tracks the source branch:
ca push  # Safe: commits are on feature branch
```

---

## 🚀 **Modern Quick Workflows**

### **Fast Feature Development**

The fastest way to build and ship features with clean commit history:

```bash
# Start feature (using shortcuts!)
ca stacks create user-auth --base main
git checkout main  # Work directly on main locally

# Rapid development - commit frequently for backup
git add . && git commit -m "WIP: start auth endpoint"
git add . && git commit -m "WIP: add validation"
git add . && git commit -m "WIP: fix bugs"
git add . && git commit -m "WIP: add tests"
git add . && git commit -m "Final: complete auth with docs"

# Squash all WIP commits into clean final commit
ca push --squash  # Auto-detects unpushed commits to squash

# Submit and auto-land when ready
ca submit            # Create PR
ca autoland          # Auto-merge when approved + builds pass

# ✅ Result: 1 clean commit, 1 PR, auto-merged when ready!
```

### **WIP to Clean Commits**

Convert messy development commits into reviewable logical units:

```bash
# Messy development (realistic!)
git commit -m "Start user model"
git commit -m "Fix typo"
git commit -m "Add email field"  
git commit -m "Validation logic"
git commit -m "More validation"
git commit -m "Tests"
git commit -m "Fix test bug"
git commit -m "Documentation"

# Intelligent squashing into logical commits  
ca push --squash 3  # Squash last 3 commits

# Submit as separate PRs for focused review
ca submit          # 3 PRs: model → validation → tests
```

### **Auto-Landing Ready PRs**

Set up automatic merging for approved changes:

```bash
# Create and populate stack
ca stacks create api-improvements --base main
git commit -m "Optimize database queries"
ca push && ca submit

git commit -m "Add request caching"  
ca push && ca submit

git commit -m "Improve error messages"
ca push && ca submit

# Auto-land all approved changes
ca autoland
# ✅ Monitors all PRs in stack
# ✅ Auto-merges when: approved + builds pass + no conflicts
# ✅ Updates dependent PRs automatically
# ✅ Handles merge order dependencies

# Check status
ca stack  # Shows PR status with auto-land indicators
```

---

## 🔄 **Complex Scenarios**



### **Base Branch Updates (Smart Force Push)**

Main branch gets updated while you're working on a feature stack:

```bash
# Your feature stack is based on old main
ca stack
# Base: main (behind by 5 commits)
# Entry 1: [abc123] Implement OAuth flow
# Entry 2: [def456] Add OAuth tests

# Smart sync with conflict detection
ca sync --check-conflicts

# Smart force push preserves all PR history:
🔄 Syncing stack: oauth-feature
   📋 Checking for conflicts with new main changes...
   ✅ No conflicts detected
   
   🔄 Rebasing using force-push strategy:
      ✅ Force-pushed implement-oauth-temp to implement-oauth (preserves PR #105)
      ✅ Force-pushed oauth-tests-temp to oauth-tests (preserves PR #106)
   
   🧹 Cleaned up 2 temporary branches
   
   ✅ Stack rebased on latest main
   ✅ All review comments and approvals preserved
   ✅ Branch names unchanged - PRs remain intact
```

### **Modifying Any Commit in Stack**

Need to change any commit in your stack? The entry editing system works for **any position** - first, middle, or last commit:

```bash
# Stack with dependencies: A -> B -> C (need to modify any entry)
ca stack  
# Entry 1: [abc123] Add database schema     (PR #110)
# Entry 2: [def456] Add user model         (PR #111) ← Need to modify this one
# Entry 3: [ghi789] Add user endpoints     (PR #112) ← depends on model

# 🆕 Modern Approach: Use entry editing (Works for ANY position!)
ca entry checkout 2  # Checkout middle entry for editing (or 1, 3, etc.)
# ✅ Automatically enters edit mode
# ✅ Checks out commit def456 safely  
# ✅ Tracks editing state

# Make your changes
git add .

# 🎯 Smart interactive guidance - just type git commit as usual:
git commit

# ⚠️ You're in EDIT MODE for a stack entry!
#
# Choose your action:
#   🔄 [A]mend: Modify the current entry
#   ➕ [N]ew:   Create new entry on top (current behavior) 
#   ❌ [C]ancel: Stop and think about it
#
# Your choice (A/n/c): A
#
# Modern approach with automatic restacking:

# Checkout the entry to edit
ca entry checkout 2  # Or use interactive: ca entry checkout

# Make your changes and amend (automatic restacking!)
ca entry amend -m "Add user model (fixed validation logic)"
# ✅ Entry #2 amended
# ✅ Entries #3, #4, #5 automatically rebased onto the changes
# ✅ Working branch updated
# ✅ Stack metadata updated

# That's it! All dependent entries are updated automatically.
# No need to run ca sync or ca rebase for dependent entries.

# Optionally sync with remote (if base branch moved forward)
ca sync  # Only needed if develop has new commits

# Check edit status anytime
ca entry status    # Shows current edit mode info
ca entry list      # Shows all entries with edit indicators

# Legacy Method (still works but requires manual ca sync):
# git checkout def456 && git commit --amend && ca sync
```

### **Managing Multiple Related Stacks**

Working on authentication feature that depends on database changes from another team:

```bash
# Create dependent stack
ca stacks create auth-endpoints --base user-database --description "Auth endpoints (depends on DB stack)"

# Visual dependency tracking
ca viz stack
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│ main           │────│ user-database   │────│ auth-endpoints  │
│ (stable)       │    │ (Team A)        │    │ (Your stack)    │
└─────────────────┘    └─────────────────┘    └─────────────────┘

# When Team A's database stack gets updated:
ca sync  # Automatic dependency resolution
# ✅ Detects user-database changes
# ✅ Rebases auth-endpoints on latest user-database
# ✅ Preserves your work while incorporating their changes

# Advanced: Cross-team coordination
ca repo  # See all team stacks
ca sync --upstream=user-database  # Explicit upstream sync
```

### **Handling Merge Conflicts During Rebase**

When automatic rebase fails due to conflicts:

```bash
ca rebase
# ❌ Merge conflict in src/auth.rs
# ❌ Rebase paused - resolve conflicts and continue

# Smart conflict resolution assistance
ca sync --resolve

# Complete the rebase
git add src/auth.rs
ca rebase --continue

# ✅ Rebase completed with smart conflict tracking
# ✅ All PRs updated with conflict resolution
# ✅ Review history preserved
```

### **Emergency Hotfix with Parallel Development**

Need to create urgent hotfix while feature work continues:

```bash
# Currently working on feature stack
ca stacks list
# * feature-oauth (active)
#   user-profiles

# Quick hotfix workflow
ca stacks create hotfix-critical-bug
# ✅ Automatically switches to hotfix context
# ✅ Preserves feature-oauth stack state

# Work on hotfix (feature-oauth stack paused)
git add . && git commit -m "Fix authentication vulnerability"
ca push
ca submit --priority high

# Fast-track approval and merge  
ca autoland --wait-for-builds
# ✅ Auto-merges as soon as approved (bypasses normal wait times)
# ✅ Sends notifications to team about urgent merge

# Switch back to feature work seamlessly
ca switch feature-oauth
# ✅ Restored exact working state
# ✅ No git stash/unstash needed

# Sync feature stack with hotfix changes
ca sync
# ✅ Automatically incorporates hotfix into feature branch
# ✅ Detects and resolves any conflicts
```

### **Stack Cleanup After Merges**

Managing stacks after some commits get merged:

```bash
# Stack with mixed merge status
ca prs  # Using shortcut!
# Entry 1: [abc123] Add user model         (PR #120 - Merged ✅)
# Entry 2: [def456] Add user validation    (PR #121 - Open)
# Entry 3: [ghi789] Add user endpoints     (PR #122 - Open)

# Automatic cleanup of merged entries
ca land --cleanup
# ✅ Detected merged PR #120
# ✅ Removed merged entries from stack
# ✅ Rebased remaining entries on latest main (includes merged changes)
# ✅ Updated dependencies automatically

# Manual cleanup for specific control
ca pop 1 --merged  # Remove only merged entries
ca rebase         # Update remaining stack

# Final clean state
ca stack
# Entry 1: [def456] Add user validation    (PR #121)
# Entry 2: [ghi789] Add user endpoints     (PR #122)
# ✅ Stack continues cleanly from merged base
```

---

## 🚧 **Future: Team Collaboration**

**Current Status**: Cascade CLI is currently **individual/local-only**. Stack metadata is stored in `.cascade/` (gitignored) and not shared between team members.

### **Why No Team Features Yet?**

The current architecture prioritizes:
- **Simplicity**: No backend infrastructure required
- **Reliability**: Works offline, no network dependencies  
- **Individual productivity**: Focus on personal workflow optimization

### **Team Collaboration Options Under Consideration**

#### **Option 1: Git-Based Sharing**
```bash
# Commit stack metadata to share with team
git add .cascade/
git commit -m "Share feature-auth stack"

# Team members can see and build on each other's stacks
ca stacks list --all-contributors
```

**Pros:** Simple, no infrastructure  
**Cons:** JSON merge conflicts, complex rebasing

#### **Option 2: Backend Service**
```bash
# Future API-based collaboration
ca stacks sync --remote
ca stacks share feature-auth --with-team backend-team
ca stacks deps --cross-team
```

**Pros:** Real-time sync, advanced conflict resolution  
**Cons:** Requires infrastructure, network dependency

#### **Option 3: Hybrid Local-First**
```bash
# Local-first with optional sync
ca stacks create feature --shared   # Opt-in to sharing
ca stacks sync                      # When network available
ca stacks work --offline           # Always works locally
```

**Pros:** Best of both worlds  
**Cons:** Most complex to implement

### **Current Workarounds for Teams**

While we design the best collaboration approach:

#### **Share via Pull Requests**
```bash
# Current workflow: Share through PRs
ca push          # Creates feature branches
ca submit        # Creates PRs
# Team reviews PRs normally through GitHub/Bitbucket
```

#### **Coordinate Base Branches**
```bash
# Team lead creates shared integration branch
git checkout -b team/integration-branch
ca stacks create my-feature --base team/integration-branch
```

#### **Stack Documentation**
```bash
# Document stack architecture in commit messages
ca push -m "Entry 1/3: Add user model (part of auth feature)"
ca push -m "Entry 2/3: Add validation (depends on user model)"
```

### **Your Input Needed!**

We're designing team collaboration features. What's most important to your team?

1. **Simple Git-based sharing** (fastest to implement)
2. **Advanced backend features** (most powerful)
3. **Hybrid approach** (most flexible)

**Share feedback**: [GitHub Discussions](https://github.com/org/cascade-cli/discussions) or `ca feedback --feature="team-collaboration"`

---

## 💡 **Pro Tips for Advanced Workflows**

### **Optimizing for Code Review**

```bash
# Create reviewer-friendly commits
ca push --logical    # Groups related changes automatically
ca submit --reviewers="@security-team" --when="auth"  # Conditional reviewers
ca submit --size=small  # Ensures commits stay review-friendly
```

### **Performance at Scale**

```bash
# Large repository optimizations
ca config set performance.lazy_loading true
ca config set performance.batch_operations true
ca stacks create large-feature --workers=4  # Parallel processing
```

### **Integration with CI/CD**

```bash
# Pipeline integration
ca hooks install --ci-mode  # Optimized for automated environments
ca submit --wait-for-ci     # Block until CI passes
ca autoland --require-green-ci  # Extra safety for Beta environments
```

These workflows showcase how Cascade CLI's modern features like shortcuts, smart sync, autoland, and conflict resolution make complex development scenarios much simpler and safer to manage. 