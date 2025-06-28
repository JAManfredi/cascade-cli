# 🔄 **Cascade CLI Workflows**

A comprehensive guide to real-world development workflows using Cascade CLI's stacked diff approach.

## 📋 **Table of Contents**

- [🌲 Understanding Git Branches vs Stacks](#understanding-git-branches-vs-stacks)
- [🚀 Modern Quick Workflows](#modern-quick-workflows)
  - [Fast Feature Development](#fast-feature-development)
  - [WIP to Clean Commits](#wip-to-clean-commits)
  - [Auto-Landing Ready PRs](#auto-landing-ready-prs)
- [🔄 Complex Scenarios](#complex-scenarios)
  - [Code Review Feedback on Middle Commit](#code-review-feedback-on-middle-commit)
  - [Base Branch Updates (Smart Force Push)](#base-branch-updates-smart-force-push)
  - [Modifying First Commit in Stack](#modifying-first-commit-in-stack)
  - [Managing Multiple Related Stacks](#managing-multiple-related-stacks)
  - [Handling Merge Conflicts During Rebase](#handling-merge-conflicts-during-rebase)
  - [Emergency Hotfix with Parallel Development](#emergency-hotfix-with-parallel-development)
  - [Stack Cleanup After Merges](#stack-cleanup-after-merges)
- [🎯 Team Collaboration Patterns](#team-collaboration-patterns)
  - [Cross-Team Dependencies](#cross-team-dependencies)
  - [Shared Infrastructure Changes](#shared-infrastructure-changes)
  - [Release Train Management](#release-train-management)

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
cc stacks create my-feature --base main

# Switch to a different Git branch
git checkout some-other-branch

# Your stack is STILL active!
cc stack  # Still shows "my-feature" stack
```

#### **2. Each Stack Has Its Own Base Branch**
```bash
# Different stacks can have different base branches
cc stacks create frontend-work --base main
cc stacks create hotfix --base release-1.2
cc stacks create experiment --base develop
```

#### **3. Stack Entries Create Individual Branches**
```bash
# When you push to a stack, Cascade creates branches automatically
cc push  # Creates feature/my-feature-1, feature/my-feature-2, etc.

# These branches are SEPARATE from your current working branch
git branch  # Shows: main, feature/my-feature-1, feature/my-feature-2
```

### **🔄 Two Common Workflow Patterns**

#### **Pattern 1: Work Directly on Base Branch (Recommended)**
```bash
git checkout main           # Start on main
cc stacks create my-feature  # Create stack based on main
# Make commits directly on main
cc push                     # Each commit becomes a stack entry with its own branch
```

#### **Pattern 2: Work on Feature Branch, Then Stack**
```bash
git checkout -b feature-branch
# Make several commits
cc stacks create my-feature --base main
cc push               # Add existing commits to stack
```

### **🤔 What Happens When You Switch Git Branches?**

**Current Behavior (Cascade CLI)**:
- Your **active stack remains the same**
- Stack state is **persisted in `.cascade/metadata.json`**
- You can work on your stack from **any Git branch**

```bash
# Start with stack active
cc stack  # Shows "my-feature" stack

# Switch Git branches
git checkout develop
git checkout -b random-experiment

# Stack is STILL active
cc stack  # Still shows "my-feature" stack
cc push  # Still adds to "my-feature" stack
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
cc stack  # Shows stack that doesn't match your current branch
```
**Solution**: Use `cc stacks switch` to explicitly change stacks.

#### **2. Commits from Wrong Branch**
```bash
git checkout feature-branch
# Commit some work
cc push  # Adds feature-branch commits to whatever stack is active
```
**Solution**: Always check `cc stack` before `cc push`.

#### **3. Multiple Stacks Confusion**
```bash
cc stacks list  # Shows multiple stacks
# Which one is active? Which branch am I on?
```
**Solution**: Use status indicators and explicit switching.

### **💡 Best Practices**

#### **1. Use Stack-Aware Status**
```bash
# Always check both Git and stack status
git status && cc stack
```

#### **2. Explicit Stack Switching**
```bash
# Don't rely on Git branch to determine stack
cc stacks switch my-other-feature
```

#### **3. Name Stacks Clearly**
```bash
# Use descriptive names that match your mental model
cc stacks create user-auth-backend --base main
cc stacks create mobile-ui-fixes --base develop
```

#### **4. One Stack Per Feature Branch (If Using Pattern 2)**
```bash
git checkout -b user-authentication
cc stacks create user-auth --base main
# Keep the feature branch and stack names aligned
```

### **🔧 Potential Improvements for Cascade CLI**

The current behavior could be enhanced with:

1. **Branch-aware default**: When creating a stack, use current branch name as default stack name
2. **Visual indicators**: Show current Git branch vs active stack in `cc stack`
3. **Auto-switch option**: Flag to auto-switch stacks when changing Git branches
4. **Stack-branch binding**: Option to bind a stack to a specific Git branch

## 🛡️ **Base Branch Protection (New!)**

**Cascade CLI now protects against accidentally polluting your base branch** with work-in-progress commits.

### **🚨 The Problem We Solve**

```bash
# ❌ DANGEROUS: Direct work on main
git checkout main
git commit -am "WIP: trying something"  # This commit is NOW ON MAIN!
cc push  # Oops, now main has WIP commits
```

### **✅ Built-in Safety Features**

#### **1. Auto-Detection & Warning**
```bash
git checkout main
cc push  # Cascade CLI detects base branch work

# Output:
# 🚨 WARNING: You're currently on the base branch 'main'
#    Making commits directly on the base branch is not recommended.
#    This can pollute the base branch with work-in-progress commits.
```

#### **2. Auto-Branch Creation**
```bash
# Let Cascade create a safe feature branch automatically
cc push --auto-branch

# Output:
# 🚀 Auto-creating feature branch 'feature/my-feature-work'...
# 🍒 Cherry-picking 2 commit(s) to new branch...
#    ✅ Cherry-picked a1b2c3d4
#    ✅ Cherry-picked e5f6g7h8
# ✅ Successfully moved 2 commit(s) to 'feature/my-feature-work'
```

#### **3. Source Branch Tracking**
Cascade CLI now tracks where each commit was originally made:

```bash
cc stack
# Output:
# 📚 Stack Entries:
#    1. a1b2c3d4 📝 Add authentication (from main)
#    2. e5f6g7h8 📝 Add validation (from feature/auth)
#    3. i9j0k1l2 📝 Add tests (from feature/auth)
```

#### **4. Manual Override (Not Recommended)**
```bash
# Force push from base branch (dangerous)
cc push --allow-base-branch
```

### **🔄 Updated Workflow Patterns**

#### **Pattern 1: Safe Feature Branch Workflow (Recommended)**
```bash
git checkout main
cc stacks create user-auth --base main

# Always work on feature branches
git checkout -b feature/authentication
git commit -am "Add login system"
git commit -am "Add password validation"

cc push  # Safe: commits are on feature branch
```

#### **Pattern 2: Auto-Branch Recovery**
```bash
# If you accidentally worked on main...
git checkout main
git commit -am "Oops, worked on main"

# Cascade CLI to the rescue:
cc push --auto-branch  # Safely moves commits to feature branch
```

#### **Pattern 3: Explicit Branch Creation**
```bash
git checkout main
cc push  # Cascade CLI warns and suggests options

# Follow the guidance:
git checkout -b feature/my-work
cc push  # Now safe
```

#### **Pattern 4: Emergency on Feature Branch**
If you happen to be on a feature branch that matches the commits you want to add:

```bash
git checkout feature/urgent-fix  
git commit -am "Fix critical bug"

# Safe because cascade tracks the source branch:
cc push  # Safe: commits are on feature branch
```

---

## 🚀 **Modern Quick Workflows**

### **Fast Feature Development**

The fastest way to build and ship features with clean commit history:

```bash
# Start feature (using shortcuts!)
cc stacks create user-auth --base main
git checkout main  # Work directly on main locally

# Rapid development - commit frequently for backup
git add . && git commit -m "WIP: start auth endpoint"
git add . && git commit -m "WIP: add validation"
git add . && git commit -m "WIP: fix bugs"
git add . && git commit -m "WIP: add tests"
git add . && git commit -m "Final: complete auth with docs"

# Squash all WIP commits into clean final commit
cc stacks push --squash  # Auto-detects unpushed commits to squash

# Submit and auto-land when ready
cc submit            # Create PR
cc autoland          # Auto-merge when approved + builds pass

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
cc stacks push --squash 3  # Squash last 3 commits

# Submit as separate PRs for focused review
cc submit          # 3 PRs: model → validation → tests
```

### **Auto-Landing Ready PRs**

Set up automatic merging for approved changes:

```bash
# Create and populate stack
cc stacks create api-improvements --base main
git commit -m "Optimize database queries"
cc push && cc submit

git commit -m "Add request caching"  
cc push && cc submit

git commit -m "Improve error messages"
cc push && cc submit

# Auto-land all approved changes
cc autoland
# ✅ Monitors all PRs in stack
# ✅ Auto-merges when: approved + builds pass + no conflicts
# ✅ Updates dependent PRs automatically
# ✅ Handles merge order dependencies

# Check status
cc stack  # Shows PR status with auto-land indicators
```

---

## 🔄 **Complex Scenarios**

### **Code Review Feedback on Middle Commit**

You have a 3-commit stack and need to modify the middle commit based on review feedback:

```bash
# Your stack: A -> B -> C (need to modify B)
cc stack
# Entry 1: [abc123] Add authentication endpoints    (PR #101 - Open)
# Entry 2: [def456] Add password validation        (PR #102 - Changes Requested) ← Need to fix
# Entry 3: [ghi789] Add user registration tests    (PR #103 - Open)

# Method 1: Direct checkout and amend (traditional)
git checkout def456
git add .
git commit --amend -m "Add password validation (addressed security review)"
cc rebase  # Update all dependent commits

# Method 2: Interactive rebase (modern approach)
git rebase -i HEAD~3   # Pick the commit to edit
# Edit the commit, then:
cc rebase              # Cascade handles the rest

# Auto-update dependent PRs
cc sync  

# ✅ Force-pushed new content to existing branches (preserves PR #101, #102, #103)
# ✅ All dependent commits automatically updated
# ✅ Review history preserved on all PRs
```

### **Base Branch Updates (Smart Force Push)**

Main branch gets updated while you're working on a feature stack:

```bash
# Your feature stack is based on old main
cc stack
# Base: main (behind by 5 commits)
# Entry 1: [abc123] Implement OAuth flow
# Entry 2: [def456] Add OAuth tests

# Smart sync with conflict detection
cc sync --check-conflicts

# Smart force push preserves all PR history:
🔄 Syncing stack: oauth-feature
   📋 Checking for conflicts with new main changes...
   ✅ No conflicts detected
   📋 Branch mapping:
      implement-oauth -> implement-oauth-v2
      oauth-tests -> oauth-tests-v2
   
   🔄 Preserved pull request history:
      ✅ Force-pushed implement-oauth-v2 to implement-oauth (preserves PR #105)
      ✅ Force-pushed oauth-tests-v2 to oauth-tests (preserves PR #106)
   
   ✅ Stack rebased on latest main
   ✅ All review comments and approvals preserved
   ✅ Backup branches created: implement-oauth-v2, oauth-tests-v2
```

### **Modifying First Commit in Stack**

Need to change the foundation commit that other commits depend on:

```bash
# Stack with dependencies: A -> B -> C (need to modify A)
cc stack  
# Entry 1: [abc123] Add database schema     (PR #110)
# Entry 2: [def456] Add user model         (PR #111) ← depends on schema
# Entry 3: [ghi789] Add user endpoints     (PR #112) ← depends on model

# Method 1: Checkout and amend foundation
git checkout abc123
git add .
git commit --amend -m "Add database schema (fixed column types)"
cc rebase  # Cascade handles all dependencies

# Method 2: Interactive rebase for multiple changes
git rebase -i HEAD~3   # Mark first commit as 'edit'
# Make changes, git add ., git commit --amend
# git rebase --continue, then:
cc rebase  # Cascade synchronizes all PRs

# ✅ All dependent commits automatically incorporate the schema changes
# ✅ All PRs (#110, #111, #112) updated with new code but preserve review history
```

### **Managing Multiple Related Stacks**

Working on authentication feature that depends on database changes from another team:

```bash
# Create dependent stack
cc stacks create auth-endpoints --base user-database --description "Auth endpoints (depends on DB stack)"

# Visual dependency tracking
cc viz stack
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│ main           │────│ user-database   │────│ auth-endpoints  │
│ (stable)       │    │ (Team A)        │    │ (Your stack)    │
└─────────────────┘    └─────────────────┘    └─────────────────┘

# When Team A's database stack gets updated:
cc sync  # Automatic dependency resolution
# ✅ Detects user-database changes
# ✅ Rebases auth-endpoints on latest user-database
# ✅ Preserves your work while incorporating their changes

# Advanced: Cross-team coordination
cc repo  # See all team stacks
cc sync --upstream=user-database  # Explicit upstream sync
```

### **Handling Merge Conflicts During Rebase**

When automatic rebase fails due to conflicts:

```bash
cc rebase
# ❌ Merge conflict in src/auth.rs
# ❌ Rebase paused - resolve conflicts and continue

# Smart conflict resolution assistance
cc sync --resolve

# Complete the rebase
git add src/auth.rs
cc rebase --continue

# ✅ Rebase completed with smart conflict tracking
# ✅ All PRs updated with conflict resolution
# ✅ Review history preserved
```

### **Emergency Hotfix with Parallel Development**

Need to create urgent hotfix while feature work continues:

```bash
# Currently working on feature stack
cc stacks list
# * feature-oauth (active)
#   user-profiles

# Quick hotfix workflow
cc stacks create hotfix-critical-bug
# ✅ Automatically switches to hotfix context
# ✅ Preserves feature-oauth stack state

# Work on hotfix (feature-oauth stack paused)
git add . && git commit -m "Fix authentication vulnerability"
cc push
cc submit --priority high

# Fast-track approval and merge  
cc autoland --wait-for-builds
# ✅ Auto-merges as soon as approved (bypasses normal wait times)
# ✅ Sends notifications to team about urgent merge

# Switch back to feature work seamlessly
cc stacks switch feature-oauth
# ✅ Restored exact working state
# ✅ No git stash/unstash needed

# Sync feature stack with hotfix changes
cc sync
# ✅ Automatically incorporates hotfix into feature branch
# ✅ Detects and resolves any conflicts
```

### **Stack Cleanup After Merges**

Managing stacks after some commits get merged:

```bash
# Stack with mixed merge status
cc prs  # Using shortcut!
# Entry 1: [abc123] Add user model         (PR #120 - Merged ✅)
# Entry 2: [def456] Add user validation    (PR #121 - Open)
# Entry 3: [ghi789] Add user endpoints     (PR #122 - Open)

# Automatic cleanup of merged entries
cc land --cleanup
# ✅ Detected merged PR #120
# ✅ Removed merged entries from stack
# ✅ Rebased remaining entries on latest main (includes merged changes)
# ✅ Updated dependencies automatically

# Manual cleanup for specific control
cc pop 1 --merged  # Remove only merged entries
cc rebase         # Update remaining stack

# Final clean state
cc stack
# Entry 1: [def456] Add user validation    (PR #121)
# Entry 2: [ghi789] Add user endpoints     (PR #122)
# ✅ Stack continues cleanly from merged base
```

---

## 🎯 **Team Collaboration Patterns**

### **Cross-Team Dependencies**

Managing features that depend on work from other teams:

```bash
# Team A working on database layer
# Team B (you) working on API layer that depends on database

# Create stack with explicit dependency
cc stacks create api-v2 --base team-a/database-refactor
cc stacks create payments --base api-v2  # Further dependency

# Team coordination features
cc repo  # See all team stacks
cc stacks deps --team="Team A"
# Shows: team-a/database-refactor (2 commits ahead, 0 behind)
# Shows: Estimated completion: 2 days (based on Team A velocity)

# Get notified of upstream changes
cc sync --watch --team="Team A"
# ✅ Monitors team-a/database-refactor for changes
# ✅ Auto-syncs your stack when their changes are ready
# ✅ Notifies about breaking changes requiring attention
```

### **Shared Infrastructure Changes**

Managing changes that affect multiple teams:

```bash
# Infrastructure change affecting 3 teams
cc stacks create auth-migration --base main --shared
cc tag add breaking-change

# Build migration with rollback plan
git commit -m "Add OAuth 2.0 support (backward compatible)"
cc push
git commit -m "Migrate existing auth tokens"
cc push  
git commit -m "Remove legacy auth (breaking change)"
cc push

# Coordinated rollout
cc submit --strategy=rolling
# ✅ Creates PR #1 (non-breaking) - can merge immediately
# ✅ Creates PR #2 (migration) - scheduled for next sprint
# ✅ Creates PR #3 (breaking) - blocked until migration complete

# Teams can prepare for changes
cc share-preview --teams="frontend,mobile,backend"
# ✅ Sends preview branches to other teams
# ✅ Enables parallel testing and adaptation
# ✅ Collects feedback before final merge
```

### **Release Train Management**

Coordinating multiple features for a scheduled release:

```bash
# Release train for Q1 features
cc stacks create q1-release --base main --release="2024.1"

# Add features from different teams to release
cc stacks merge feature-auth --target=q1-release
cc stacks merge feature-search --target=q1-release  
cc stacks merge feature-billing --target=q1-release

# Release coordination
cc release plan q1-release
# 📋 Feature readiness:
#   ✅ feature-auth: Ready (approved, tested)
#   ⚠️  feature-search: Waiting for QA approval
#   ❌ feature-billing: Failing integration tests

# Selective release if needed
cc release deploy --features="auth,search" --exclude="billing"
# ✅ Deploys ready features
# ✅ Keeps failing features in development
# ✅ Maintains clean release history

# Monitor release health
cc release status q1-release
# 📊 Deployment: 95% success rate
# 📊 Performance: +2% improvement
# 📊 Rollback plan: Ready if needed
```

---

## 💡 **Pro Tips for Advanced Workflows**

### **Optimizing for Code Review**

```bash
# Create reviewer-friendly commits
cc push --logical    # Groups related changes automatically
cc submit --reviewers="@security-team" --when="auth"  # Conditional reviewers
cc submit --size=small  # Ensures commits stay review-friendly
```

### **Performance at Scale**

```bash
# Large repository optimizations
cc config set performance.lazy_loading true
cc config set performance.batch_operations true
cc stacks create large-feature --workers=4  # Parallel processing
```

### **Integration with CI/CD**

```bash
# Pipeline integration
cc hooks install --ci-mode  # Optimized for automated environments
cc submit --wait-for-ci     # Block until CI passes
cc autoland --require-green-ci  # Extra safety for Beta environments
```

These workflows showcase how Cascade CLI's modern features like shortcuts, smart sync, autoland, and conflict resolution make complex development scenarios much simpler and safer to manage. 