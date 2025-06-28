# Edit Flows Integration - How Smart Force Push Strategy Works

## Overview

Cascade CLI uses a **smart force push strategy** to handle the complex workflow of stacked diffs when the base branch changes. This approach **preserves ALL review history** while updating branch contents, following industry best practices established by tools like Graphite, Phabricator, and GitHub CLI.

## The Challenge: Stacked Diffs + Base Branch Updates

When working with stacked diffs, a fundamental problem occurs when the base branch (like `main`) gets updated with new commits:

```
Before base update:
main: A -> B
feature-1: A -> B -> C (PR #1)
feature-2: A -> B -> C -> D (PR #2)

After main gets updated:
main: A -> B -> E -> F
feature-1: A -> B -> C (now behind by 2 commits)
feature-2: A -> B -> C -> D (now behind by 2 commits)
```

## ❌ **BAD Solution: Branch Versioning**

```bash
# This approach creates NEW PRs and LOSES review history
feature-1: A -> B -> C (PR #1 - ORIGINAL)
feature-1-v2: A -> B -> E -> F -> C' (PR #123 - NEW, no history)
feature-2-v2: A -> B -> E -> F -> C' -> D' (PR #124 - NEW, no history)
```

**Problems:**
- ❌ Lost review comments, discussions, approvals
- ❌ Broken review thread continuity  
- ❌ Reviewers confused about which PR to look at
- ❌ Lost approval history and status
- ❌ Team notification spam with duplicate PRs

## ✅ **SMART Solution: Force Push with PR History Preservation**

### How Industry Leaders Do It

**Graphite CLI** (Industry Standard):
```bash
# After rebase, Graphite literally does:
git push origin feature-branch --force
```

**Phabricator/Meta** (The Pioneer):
- Uses the **same review object** when commits change
- Preserves ALL review history, comments, discussions
- Has "sticky acceptance" - approved reviews stay approved

**GitHub CLI** (`gh pr sync`):
- Force pushes to existing branches to preserve PR URLs and history
- Reviewers see clear diff updates in the same conversation thread

### Our Smart Force Push Implementation

```rust
// When rebasing creates new branch content:
let new_branch = "feature-auth-v2";  // Temporary rebase branch
let existing_branch = "feature-auth"; // Original PR branch

// Instead of creating new PR, we preserve the old one:
git_repo.force_push_branch(&existing_branch, &new_branch)?;
// This updates 'feature-auth' content with 'feature-auth-v2' content
// The PR #123 still points to 'feature-auth' but now has updated content!
```

## What Happens During Rebase with Smart Force Push:

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

📝 Review workflow preserved:
   - PR #123: Still points to 'add-auth' branch with updated content
   - PR #124: Still points to 'add-tests' branch with updated content
   - All comments, approvals, discussions PRESERVED
   - Reviewers get notified of content updates in same thread
```

## Benefits of Smart Force Push Strategy

### ✅ **Complete Review History Preservation**
- All comments, discussions, inline reviews stay intact
- Approval history maintained
- Review thread continuity preserved
- No duplicate PRs or confusion

### ✅ **Industry-Standard Approach**
- Same strategy used by Graphite (300k+ users)
- Proven by Meta/Phabricator (billions of commits)
- Supported by GitHub CLI and major platforms

### ✅ **Reviewer-Friendly Experience**
- Reviewers see updates in familiar PR interface
- Clear "changes requested" → "updated" flow
- No need to track multiple PR numbers
- Notification continuity in same thread

### ✅ **Team Workflow Benefits**
- CI/CD pipelines continue on same PR
- Branch protection rules stay consistent
- PR URLs remain stable for bookmarks/links
- Integration tools (Slack, JIRA) track same PR

## Technical Implementation Details

### 1. **Automatic Detection**
```rust
if let (Some(pr_id), Some(new_branch)) = 
   (&entry.pull_request_id, branch_mapping.get(&entry.branch)) {
   // This entry has an existing PR and was rebased to new branch
}
```

### 2. **Smart Force Push**
```rust
// Force push new content to preserve PR history
git_repo.force_push_branch(&entry.branch, new_branch)?;
// entry.branch (e.g., 'feature-auth') now has content from new_branch
```

### 3. **Safe Operations**
- Validates PR exists before attempting update
- Falls back gracefully if force push fails
- Preserves original branches as backup
- Reports success/failure clearly to user

### 4. **Authentication Handling**
- Uses existing git credentials (SSH keys, tokens)
- Leverages git credential manager
- Same auth as normal git operations

## Comparison with Alternative Approaches

| Approach | Review History | PR URLs | Complexity | Industry Usage |
|----------|---------------|---------|------------|----------------|
| **Smart Force Push** | ✅ Preserved | ✅ Stable | 🟡 Medium | ✅ Standard |
| Branch Versioning | ❌ Lost | ❌ New URLs | 🔴 High | ❌ None |
| Manual Rebasing | ✅ Preserved | ✅ Stable | 🔴 Very High | 🟡 Some |
| No Rebasing | ❌ Outdated | ✅ Stable | 🟢 Low | ❌ Not viable |

## Safety and Recovery

### Built-in Safety Measures
- **Original branches preserved**: `feature-v2` branches kept as backup
- **Atomic operations**: Either all force pushes succeed or none do
- **Clear error reporting**: User knows exactly what succeeded/failed
- **Manual recovery options**: User can always restore from backup branches

### If Something Goes Wrong
```bash
# Restore from backup if needed:
git checkout add-auth-v2
git branch -D add-auth
git checkout -b add-auth
git push origin add-auth --force

# Or just create new PR manually:
cc stacks submit --title "Updated feature after rebase"
```

## Why This Approach Wins

**The key insight**: In stacked diff workflows, **review continuity is more valuable than avoiding force pushes**.

- **Phabricator** proved this at Meta scale (tens of thousands of engineers)
- **Graphite** adopted it and scaled to hundreds of companies
- **GitHub CLI** implements it for the same reasons

**Force pushes are safe when:**
1. They're to feature branches (not main/master)
2. They preserve review context and team workflows  
3. They're done atomically with proper error handling
4. They follow established authentication patterns

This is why Cascade CLI adopted the smart force push strategy - it provides the best developer and reviewer experience while maintaining the safety and reliability needed for Beta workflows. 