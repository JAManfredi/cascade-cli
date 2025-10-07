# Data Loss Recovery Guide

If you lost changes due to the auto-resolve bug (fixed in v0.1.74), follow these steps:

## Step 1: Find Your Lost Commit

```bash
cd /path/to/your/repo

# Search reflog for your commit hash (you saw this in "Updated metadata")
git reflog | grep "fac186c3"

# Or search by commit message
git reflog | grep -i "your commit message"

# Or just look at recent HEAD movements
git reflog | head -20
```

## Step 2: Verify the Commit Has Your Changes

```bash
# Show the commit content (replace with your hash)
git show fac186c3

# This should show your changes!
```

## Step 3: Recover Your Changes

### Option A: Cherry-pick the commit back

```bash
# Checkout your branch
git checkout feat-add-base-component-class

# Cherry-pick your lost commit
git cherry-pick fac186c3

# Update the stack metadata
ca entry amend --all --restack
```

### Option B: Reset your branch to the good commit

```bash
# Checkout your branch
git checkout feat-add-base-component-class

# Hard reset to your good commit
git reset --hard fac186c3

# Update the stack metadata
ca push  # This will update the stack entry with the correct hash
```

### Option C: Create a new branch from the good commit

```bash
# Create a new branch from your good commit
git checkout -b feat-add-base-component-class-recovered fac186c3

# Delete the old branch
git branch -D feat-add-base-component-class

# Rename the recovered branch
git branch -m feat-add-base-component-class

# Update stack
ca push
```

## Step 4: Check for Backup Files

```bash
# Look for .cascade-backup files
find . -name "*.cascade-backup" -type f

# These contain original file content before auto-resolution
# Compare with current files to see what was changed
```

## Step 5: Force Push to Update PR

```bash
# After recovering your changes
git push --force-with-lease origin feat-add-base-component-class

# Or use cascade
ca sync  # With the fixed version, this should work correctly now
```

## Prevention

Update to cascade v0.1.74 or later, which fixes the auto-resolve bugs:

```bash
brew upgrade cascade-cli
cd /path/to/your/repo
ca hooks uninstall --all
ca hooks install
```

## If Reflog Doesn't Have It

If the commit is not in reflog, check:

1. **Your working branch**: `git log SBN-18037/create-base-component-classes`
2. **Remote backup**: `git log origin/feat-add-base-component-class`
3. **Bitbucket PR**: The PR might still have the old commit visible in its history

## Need Help?

If you can't recover your changes, provide:
- Output of `git reflog | head -30`
- The commit hashes you saw in the "Updated metadata" message
- Your branch name

We can help you recover it!
