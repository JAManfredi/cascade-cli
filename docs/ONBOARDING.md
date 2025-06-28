# 🎓 Onboarding Guide

Welcome to Cascade CLI! This guide will take you from zero to productive in 15 minutes with hands-on tutorials and real examples.

## 🎯 **What You'll Learn**

By the end of this guide, you'll be able to:
- ✅ Set up Cascade CLI in your repository
- ✅ Create and manage stacked diffs
- ✅ Submit organized pull requests
- ✅ Use advanced features like TUI and visualizations
- ✅ Integrate with your team's workflow

---

## 📚 **Prerequisites**

Before starting, ensure you have:
- **Git repository** with remote access
- **Bitbucket Server/Cloud** account with Personal Access Token
- **Cascade CLI** installed ([Installation Guide](./INSTALLATION.md))

**5 minutes to verify:**
```bash
# Check prerequisites
git --version        # Should be 2.20+
cc --version         # Should show Cascade CLI version
git remote -v        # Should show your Bitbucket remote
```

---

## 🚀 **Quick Start (5 minutes)**

### **Step 1: Initialize Your Repository**
```bash
# Navigate to your Git repository
cd my-project

# Run the setup wizard (recommended)
cc setup
```

The setup wizard will:
1. 🔍 Detect your Git repository
2. 🌐 Auto-discover Bitbucket settings from remotes
3. 🔑 Guide you through token configuration
4. 🧪 Test your connection
5. ⚡ Install shell completions

### **Step 2: Create Your First Stack**
```bash
# Create a new stack for your feature
cc stacks create my-first-feature --base main --description "Learning stacked diffs"

# Make a simple change
echo "# My Feature" > FEATURE.md
git add FEATURE.md
git commit -m "Add feature documentation"

# Add commit to stack
cc stacks push

# Check status
cc repo
```

### **Step 3: Submit Your First PR**
```bash
# Submit the commit as a pull request
cc stacks submit

# Check what happened
cc stacks status
```

🎉 **Congratulations!** You've created your first stacked diff. The commit is now a pull request ready for review.

---

## 🎨 **Hands-On Tutorial (30 minutes)**

Let's build a realistic feature using stacked diffs to see the power of the workflow.

### **Scenario: Building a User Authentication System**

We'll implement user authentication in logical, reviewable increments:

#### **Phase 1: Setup (5 minutes)**

```bash
# Start fresh from main branch
git checkout main
git pull origin main

# Create our feature stack
cc stacks create user-auth --base main --description "Complete user authentication system"

# Verify we're set up correctly
cc stack
```

#### **Phase 2: Database Layer (10 minutes)**

```bash
# Create user model
mkdir -p src/models
cat << 'EOF' > src/models/user.py
class User:
    def __init__(self, username, email, password_hash):
        self.username = username
        self.email = email
        self.password_hash = password_hash
    
    def verify_password(self, password):
        # Placeholder for password verification
        return self.password_hash == hash(password)
EOF

git add src/models/user.py
git commit -m "Add User model with password verification"

# Add to stack
cc stacks push

# Create database schema
cat << 'EOF' > migrations/001_create_users.sql
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    username VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(100) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
EOF

git add migrations/001_create_users.sql
git commit -m "Add user table migration"

# Add to stack
cc stacks push

# Submit database layer for review
cc stacks submit 1 --title "Add User model" --description "Core user model with password verification"
cc stacks submit 2 --title "Add user database schema" --description "Migration to create users table"

# Check our progress
cc stack
```

#### **Phase 3: Authentication Logic (10 minutes)**

```bash
# Create authentication service
mkdir -p src/services
cat << 'EOF' > src/services/auth.py
from models.user import User
import hashlib

class AuthService:
    def __init__(self, db):
        self.db = db
    
    def register_user(self, username, email, password):
        password_hash = hashlib.sha256(password.encode()).hexdigest()
        user = User(username, email, password_hash)
        return self.db.save(user)
    
    def login(self, username, password):
        user = self.db.find_user_by_username(username)
        if user and user.verify_password(password):
            return self.generate_token(user)
        return None
    
    def generate_token(self, user):
        # Placeholder for JWT token generation
        return f"token_for_{user.username}"
EOF

git add src/services/auth.py
git commit -m "Add authentication service with login/register"

cc stacks push

# Add JWT token handling
cat << 'EOF' > src/services/token.py
import jwt
from datetime import datetime, timedelta

class TokenService:
    def __init__(self, secret_key):
        self.secret_key = secret_key
    
    def generate_token(self, user_id):
        payload = {
            'user_id': user_id,
            'exp': datetime.utcnow() + timedelta(hours=24)
        }
        return jwt.encode(payload, self.secret_key, algorithm='HS256')
    
    def verify_token(self, token):
        try:
            payload = jwt.decode(token, self.secret_key, algorithms=['HS256'])
            return payload['user_id']
        except jwt.ExpiredSignatureError:
            return None
EOF

git add src/services/token.py
git commit -m "Add JWT token service for session management"

cc stacks push

# Submit authentication layer
cc stacks submit 3 --title "Add authentication service" --description "Core login/register functionality"
cc stacks submit 4 --title "Add JWT token service" --description "Session management with secure tokens"

# Visualize our stack
cc viz stack
```

#### **Phase 4: API Endpoints (10 minutes)**

```bash
# Create API endpoints
mkdir -p src/api
cat << 'EOF' > src/api/auth_routes.py
from flask import Flask, request, jsonify
from services.auth import AuthService
from services.token import TokenService

app = Flask(__name__)
auth_service = AuthService()
token_service = TokenService('your-secret-key')

@app.route('/api/register', methods=['POST'])
def register():
    data = request.json
    try:
        user = auth_service.register_user(
            data['username'], 
            data['email'], 
            data['password']
        )
        token = token_service.generate_token(user.id)
        return jsonify({'token': token, 'user': user.username})
    except Exception as e:
        return jsonify({'error': str(e)}), 400

@app.route('/api/login', methods=['POST'])
def login():
    data = request.json
    token = auth_service.login(data['username'], data['password'])
    if token:
        return jsonify({'token': token})
    return jsonify({'error': 'Invalid credentials'}), 401

@app.route('/api/profile', methods=['GET'])
def profile():
    token = request.headers.get('Authorization', '').replace('Bearer ', '')
    user_id = token_service.verify_token(token)
    if user_id:
        # Get user profile
        return jsonify({'user_id': user_id, 'status': 'authenticated'})
    return jsonify({'error': 'Unauthorized'}), 401
EOF

git add src/api/auth_routes.py
git commit -m "Add authentication API endpoints"

cc stacks push

# Add API documentation
cat << 'EOF' > docs/api/authentication.md
# Authentication API

## Register User
POST /api/register
```json
{
  "username": "john_doe",
  "email": "john@example.com", 
  "password": "secure_password"
}
```

## Login
POST /api/login
```json
{
  "username": "john_doe",
  "password": "secure_password"
}
```

## Get Profile
GET /api/profile
Headers: Authorization: Bearer <token>
EOF

git add docs/api/authentication.md
git commit -m "Add API documentation for authentication endpoints"

cc stacks push

# Submit API layer
cc stacks submit 5 --title "Add authentication API endpoints" --description "REST API for login, register, and profile"
cc stacks submit 6 --title "Add API documentation" --description "Complete documentation for auth endpoints"
```

#### **Phase 5: Review Your Work**

```bash
# Show complete stack
cc stack

# Visualize with dependencies
cc viz stack --format mermaid

# Check all PRs
cc stacks prs

# Launch interactive TUI to explore
cc tui
```

**What you've accomplished:**
- ✅ Built a complete feature in **6 logical, reviewable pieces**
- ✅ Each PR has a clear purpose and scope
- ✅ Dependencies are properly managed
- ✅ Documentation is included and organized
- ✅ Ready for parallel review by different team members

---

## 🎯 **Real-World Scenarios**

### **Scenario 1: Handling Review Feedback**

Your reviewer wants changes to the User model. Here's how to handle it:

```bash
# Switch to the relevant commit
git checkout <commit-hash-for-user-model>

# Make the requested changes
# Edit src/models/user.py with improvements

git add src/models/user.py
git commit -m "Address review feedback: improve password validation"

# Update the existing PR
cc stacks submit 1 --title "Add User model (updated)" --description "Core user model with improved password validation"

# Sync dependent PRs if needed
cc stacks sync
```

### **Scenario 2: Dependency Changes**

The database team updated the schema. Here's how to adapt:

```bash
# Pull latest changes
git checkout main
git pull origin main

# Sync your stack with new base
cc stacks sync

# Resolve any conflicts
# Git will guide you through conflict resolution

# Continue after resolving conflicts
cc stacks rebase --continue

# Update affected PRs
cc stacks submit 2 --title "Add user database schema (updated for new DB version)"
```

### **Scenario 3: Parallel Development**

Another developer is working on related features:

```bash
# Check what other stacks exist
cc stacks list

# Visualize all dependencies
cc viz deps --format mermaid --output team-deps.md

# Create dependent stack
cc stacks create user-profiles --base user-auth --description "User profile management (depends on auth)"

# Your stack will automatically be rebased when user-auth merges
```

### **🔄 Understanding Smart Force Push (Important!)**

When you run `cc stacks rebase`, Cascade CLI uses a **smart force push strategy** that preserves all your PR history:

```bash
# When you rebase...
cc stacks rebase

# What happens behind the scenes:
# 1. Creates temporary branches: add-auth-v2, add-tests-v2  
# 2. Force pushes new content to original branches: add-auth, add-tests
# 3. All existing PRs keep their URLs, comments, and approval history!

# You'll see output like:
🔄 Rebasing stack: authentication
   ✅ Force-pushed add-auth-v2 content to add-auth (preserves PR #123)
   ✅ Force-pushed add-tests-v2 content to add-tests (preserves PR #124)
```

**Why this matters:**
- ✅ **Reviewers never lose context** - All comments and discussions preserved
- ✅ **PR URLs stay stable** - Bookmarks and links keep working  
- ✅ **Approval history maintained** - No need to re-approve unchanged code
- ✅ **Industry standard approach** - Same strategy as Graphite, Phabricator, GitHub CLI

**This is safe because:**
- Cascade CLI validates all operations before executing
- Versioned branches are kept as backup (`add-auth-v2`)
- Only affects your feature branches, never main/develop
- Atomic operations: either all updates succeed or none do

---

## 🚀 **Advanced Features Tour**

### **Terminal User Interface (TUI)**
```bash
# Launch interactive stack browser
cc tui

# Navigate with keyboard:
# ↑/↓ - Move between stacks
# Enter - Activate stack
# r - Refresh
# q - Quit
```

### **Git Hooks Integration**
```bash
# Install automation hooks
cc hooks install

# Now commits are automatically added to active stack
git commit -m "Auto-added to stack!"
# No need to run `cc stacks push`

# Check hook status
cc hooks status
```

### **Advanced Visualizations**
```bash
# ASCII art in terminal
cc viz stack

# Export for documentation
cc viz deps --format mermaid --output docs/architecture.md

# Generate diagrams for presentations
cc viz stack --format dot --output stack.dot
dot -Tpng stack.dot -o stack.png
```

### **Shell Completions**
```bash
# Install completions
cc completions install

# Now you can tab-complete:
cc stack <TAB>        # Shows: create, list, show, switch, etc.
cc stacks create <TAB> # Shows available options
```

---

## 👥 **Team Integration Patterns**

### **For Individual Contributors**

**Daily Workflow:**
```bash
# Start of day: sync with team
git checkout main && git pull
cc stacks list  # See what you're working on

# Work on features
cc stacks switch current-feature
# ... make commits ...
cc stacks push  # Add to stack
cc stacks submit  # Create PRs

# End of day: check status
cc repo  # See what's pending review
```

### **For Team Leads**

**Stack Review Process:**
```bash
# Review team's work
cc stacks list --verbose  # See all stacks
cc viz deps --format mermaid  # Understand dependencies

# Check PR status across team
cc stacks prs --format json | jq '.[] | select(.status == "open")'
```

### **For Release Management**

**Pre-release Validation:**
```bash
# Validate all stacks
cc stacks list --format name | xargs -I {} cc stacks validate {}

# Generate release documentation
cc viz deps --format mermaid > docs/release-dependencies.md
```

---

## 🔧 **Customization for Your Team**

### **Configuration Templates**

Create a team configuration template:

```bash
# .cascade/config.toml template for your team
cat << 'EOF' > .cascade/team-config.toml
[bitbucket]
url = "https://bitbucket.yourcompany.com"
project = "YOUR_PROJECT"

[workflow]
require_pr_template = true
default_reviewers = ["team-lead", "senior-dev"]
auto_submit = false

[ui]
colors = true
emoji = true

[hooks]
post_commit = true
pre_push = true
commit_msg = true
EOF

# Share with team
cp .cascade/team-config.toml .cascade/config.toml
git add .cascade/config.toml
git commit -m "Add team Cascade CLI configuration"
```

### **Git Hooks for Team Standards**

```bash
# Install hooks that enforce team standards
cc hooks install

# Customize commit message format
cc config set hooks.commit_msg_format "[TICKET-ID] Brief description"
```

---

## 🎓 **Next Steps**

### **Mastery Checklist**

After completing this guide, you should be able to:

- [ ] Create and manage stacks efficiently
- [ ] Handle review feedback with confidence
- [ ] Use advanced visualization features
- [ ] Integrate with team workflows
- [ ] Troubleshoot common issues

### **Advanced Learning**

1. **Read the [User Manual](./USER_MANUAL.md)** for complete command reference
2. **Explore [Configuration Guide](./CONFIGURATION.md)** for advanced customization
3. **Check [Architecture Guide](./ARCHITECTURE.md)** to understand internals
4. **Join community discussions** for tips and best practices

### **Practice Projects**

Try these exercises to build confidence:

1. **Multi-feature development**: Create 3 parallel stacks with different base branches
2. **Dependency management**: Create a stack that depends on another stack
3. **Conflict resolution**: Intentionally create conflicts and practice resolution
4. **Team simulation**: Work with colleagues using shared stacks

---

## 🎯 **Key Takeaways**

### **Stacked Diffs Benefits**
- **Faster reviews**: Small, focused PRs get reviewed quickly
- **Better quality**: Incremental feedback improves code
- **Parallel work**: Don't wait for reviews to continue development
- **Cleaner history**: Logical commits that tell a story

### **Best Practices**
- **One concept per commit**: Each commit should represent a single logical change
- **Descriptive messages**: Write commit messages that explain the "why"
- **Regular syncing**: Keep your stacks up-to-date with base branches
- **Use visualizations**: Diagrams help team understand dependencies

### **Common Patterns**
- **Feature development**: Break large features into logical increments
- **Bug fixes**: Separate investigation, fix, and tests
- **Refactoring**: Incremental improvements with safety
- **Documentation**: Include docs with relevant code changes

---

## 🆘 **Getting Help**

If you get stuck:

1. **Check built-in help**: `cc --help` or `cc <command> --help`
2. **Run diagnostics**: `cc doctor` to identify issues
3. **Read documentation**: [User Manual](./USER_MANUAL.md) has detailed examples
4. **Search issues**: [GitHub Issues](https://github.com/JAManfredi/cascade-cli/issues)
5. **Ask the community**: [GitHub Discussions](https://github.com/JAManfredi/cascade-cli/discussions)

---

🎉 **Welcome to the world of efficient Git workflows!** You're now equipped to handle complex development scenarios with confidence and clarity.

*Next: Explore the [User Manual](./USER_MANUAL.md) for complete command reference.* 