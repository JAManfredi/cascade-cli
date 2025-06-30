# Git Hooks Guide

Cascade CLI provides Git hooks to seamlessly integrate stack management with your regular Git workflow. This guide explains each hook in detail, when to use them, and what manual steps you'd need if hooks were disabled.

## 🎯 Key Concept: Hooks vs CLI Protections

**Git Hooks** protect against **raw Git commands** (`git commit`, `git push`)  
**CLI Commands** have built-in protections for **Cascade commands** (`csc submit`, `csc push`)

Hooks ensure that even when developers use native Git commands, they still get Cascade's benefits and protections.

## 🪝 Available Hooks

### ✅ **Installed by Default**

#### 🛡️ **Pre-Push Hook** 
*Prevents stack-breaking Git operations*

**What it does:**
```bash
# Someone tries this dangerous command:
git push --force origin feature-branch

# Hook blocks it and provides guidance:
# ❌ Force push detected!
# 🌊 Cascade CLI uses stacked diffs - force pushes can break stack integrity
# 💡 Instead try: csc sync, csc push, csc submit
```

**Manual equivalent (hooks OFF):**
```bash
# Without hooks, this would succeed and potentially break stacks:
git push --force origin feature-branch  # 💥 Could corrupt stack metadata!

# You'd need to manually remember:
csc stacks validate  # Check before any git push
```

**Why it's critical:**
- **Prevents stack corruption**: Force pushes can break stack dependency chains
- **Educational**: Teaches developers the Cascade way
- **Safety net**: Catches accidents before they cause damage

---

#### 📝 **Commit-Msg Hook** 
*Validates commit message quality*

**What it does:**
```bash
# You try to commit this:
git commit -m "fix"

# Hook blocks it:
# ❌ Commit message too short (minimum 10 characters)
# 💡 Write a descriptive commit message for better stack management
```

**Manual equivalent (hooks OFF):**
```bash
git commit -m "fix"  # Would succeed with poor message
# Later when creating PRs, you'd have poor titles/descriptions
# You'd need to manually check every commit message
```

**Why it's valuable:**
- **PR quality**: Better commit messages = better auto-generated PR titles
- **Code history**: Improves long-term maintainability
- **Early feedback**: Catches issues at commit time, not submission time

---

#### ✍️ **Prepare-Commit-Msg Hook** 
*Adds stack context to commits*

**What it does:**
```bash
# You run:
git commit

# Your editor opens with:
# 
# Stack: feature-auth
# This commit will be added to the active stack automatically.
# Use 'csc stacks status' to see the current stack state.
```

**Manual equivalent (hooks OFF):**
```bash
git commit  # Plain editor, no context
# You'd need to manually remember:
# - Which stack you're working on
# - How to check stack status
# - Whether this commit should be in a stack
```

**Why it's helpful:**
- **Context awareness**: Reminds developers which stack they're on
- **Reduces confusion**: Clear guidance about what will happen
- **Documentation**: Commits include stack context automatically

---

### 🔧 **Optional Hook (Manual Install)**

#### 📌 **Post-Commit Hook** 
*Auto-adds commits to your active stack*

**What it does:**
```bash
# You run this:
git commit -m "Fix authentication bug"

# Hook automatically runs this behind the scenes:
csc stacks push --commit [that-commit-hash] --message "Fix authentication bug"
```

**Manual equivalent (hooks OFF):**
```bash
git commit -m "Fix authentication bug"
csc push --commit $(git rev-parse HEAD)  # You'd have to remember this every time!
```

**Why it's optional:**
- ⚠️ **Conflict risk**: If your repo has existing post-commit hooks that modify files (linting, formatting), this creates a race condition
- 🔄 **Workflow pollution**: Would require `git commit --amend` after repo hooks run, disrupting stack tracking
- 🎯 **Use case**: Best for repos without conflicting post-commit hooks

**Install if needed:**
```bash
csc hooks install post-commit
```

## 🔄 Complete Workflow Comparison

### With Default Hooks (Seamless):
```bash
git commit -m "Add user authentication"
# ✅ Message validated
# ✅ Stack context included
# ⚠️ NOT auto-added to stack (manual csc push needed)

git push origin feature-branch
# ✅ Stack integrity validated
# ✅ No force push allowed
```

### With All Hooks (Full Automation):
```bash
git commit -m "Add user authentication"
# ✅ Auto-added to active stack
# ✅ Message validated
# ✅ Stack context included

git push origin feature-branch
# ✅ Stack integrity validated
# ✅ No force push allowed
```

### Without Hooks (Manual Steps):
```bash
git commit -m "Add user authentication"
csc push --commit $(git rev-parse HEAD)  # Manual step #1
csc stacks validate                        # Manual step #2

git push origin feature-branch
# 💥 Could accidentally use --force and break stacks
```

## 🛠️ Hook Management

### Install Essential Hooks (Recommended)
```bash
csc hooks install
```

### Install Specific Hook
```bash
csc hooks install post-commit    # Only if no conflicting repo hooks
csc hooks install pre-push       # Stack protection
csc hooks install commit-msg     # Message validation
csc hooks install prepare-commit-msg  # Stack context
```

### Check Hook Status
```bash
csc hooks status
```

### Remove Hooks
```bash
csc hooks uninstall              # Remove all
csc hooks uninstall post-commit  # Remove specific hook
```

## 🚨 Troubleshooting

### Conflicting Post-Commit Hooks
**Problem:** Your repo has existing post-commit hooks that modify files.

**Solution:** 
1. Don't install the post-commit hook
2. Use manual workflow: `git commit` then `csc push`
3. Or chain hooks properly (advanced - see your repo's hook documentation)

### Hook Not Running
**Problem:** Hook seems to be ignored.

**Solution:**
```bash
# Check if hooks are executable (Unix/Mac)
ls -la .git/hooks/

# Verify Cascade hooks are installed
csc hooks status

# Reinstall if needed
csc hooks install --force
```

### Force Push Still Blocked
**Problem:** Need to force push for legitimate reasons.

**Solution:**
```bash
# Use Git directly with explicit flags
git push --force-with-lease origin branch-name

# Or temporarily uninstall pre-push hook
csc hooks uninstall pre-push
git push --force origin branch-name
csc hooks install pre-push
```

## 📈 Recommendation

**Start with default hooks:**
- ✅ Pre-push: Essential for stack integrity
- ✅ Commit-msg: Improves code quality
- ✅ Prepare-commit-msg: Helpful context

**Add post-commit later if:**
- ❌ No conflicting repo hooks
- ✅ Want full automation
- ✅ Comfortable with potential conflicts 