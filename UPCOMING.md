# 🚀 Upcoming Features

This document tracks planned features and improvements for Cascade CLI that are not yet implemented but are in development or planned for future releases.

## ✅ **Recently Implemented**

### **Smart Conflict Resolution** (✅ **Completed**)
Smart automatic conflict resolution is now **fully implemented** with 4 strategies:

- **✅ Whitespace Conflicts**: Auto-resolves conflicts that only differ by whitespace
- **✅ Line Ending Conflicts**: Normalizes CRLF/LF differences  
- **✅ Pure Addition Conflicts**: Merges non-overlapping additions intelligently
- **✅ Import Reordering**: Sorts and merges import statements in common file types (Rust, Python, JS/TS, Go, Java)

**Status**: ✅ **Implemented and Available**  
**How to Use**: Enabled by default in `cc stack rebase` - conflicts are auto-resolved when possible  
**Benefits**: Reduces manual intervention in routine rebases by 60-80% in typical workflows

This feature has been moved to the main README documentation!

## 📦 **Distribution & Installation**

### **Pre-built Binaries**
Currently, Cascade CLI requires building from source. Pre-built binaries are planned for major platforms:

```bash
# macOS/Linux (Planned)
curl -L https://github.com/JAManfredi/cascade-cli/releases/latest/download/cc | sh

# Windows (Planned)
# Download from GitHub releases
```

**Status**: 🔄 In Planning
**Priority**: High  
**Estimated Release**: Next major version

## 🤖 **Advanced Conflict Resolution**

### **ML-Assisted Conflict Resolution**
Further enhancements to conflict resolution beyond the currently implemented strategies:

- **Pattern Recognition**: Machine learning models to identify and resolve complex conflict patterns
- **Project-Specific Rules**: Learning from past conflict resolutions in the same repository
- **Semantic Conflict Detection**: Understanding code semantics to resolve logical conflicts
- **Custom Resolution Strategies**: User-defined plugins for domain-specific conflict types

**Status**: 🔄 Research phase
**Priority**: Low  
**Estimated Release**: Future version

**Note**: Basic conflict resolution (whitespace, line endings, imports, pure additions) is already implemented!

## 📈 **Analytics & Reporting**

### **Stack Analytics**
- Review time tracking per stack entry
- Conflict resolution statistics
- Team collaboration metrics
- Performance insights

**Status**: 🔄 Planned
**Priority**: Low
**Estimated Release**: Future version

## 🔧 **Advanced Git Integration**

### **Git Worktree Support**
Support for Git worktrees to allow working on multiple stacks simultaneously:

```bash
cc stack create --worktree feature-auth
cc stack create --worktree bug-fix-123
```

**Status**: 🔄 Research phase
**Priority**: Medium
**Estimated Release**: Future version

## 🌐 **Multi-Platform Support**

### **GitHub Integration**
While designed for Bitbucket Server, GitHub support is planned:

```bash
cc init --github-url https://github.com/owner/repo
```

**Status**: 🔄 Architecture allows, not implemented
**Priority**: Medium
**Estimated Release**: Future version

### **GitLab Integration**
GitLab support following the same pattern as GitHub:

```bash
cc init --gitlab-url https://gitlab.com/owner/repo
```

**Status**: 🔄 Planned
**Priority**: Low
**Estimated Release**: Future version

## 📱 **Developer Experience**

### **IDE Integrations**
- VS Code extension
- IntelliJ/JetBrains plugin
- Vim/Neovim plugin

**Status**: 🔄 Planned
**Priority**: Low
**Estimated Release**: Future version

### **Web Dashboard**
Optional web interface for stack management and PR tracking:

```bash
cc server start --port 8080
# Opens web dashboard at http://localhost:8080
```

**Status**: 🔄 Concept phase
**Priority**: Low
**Estimated Release**: Future version

---

## 🎯 **How to Contribute**

Interested in implementing any of these features? Check out our [Contributing Guide](CONTRIBUTING.md) and:

1. **Pick a feature** from this list
2. **Open an issue** to discuss approach
3. **Submit a PR** with your implementation

## 📅 **Release Planning**

- **Next Patch**: Bug fixes and small improvements
- **Next Minor**: Pre-built binaries, advanced conflict resolution
- **Next Major**: Multi-platform support, advanced integrations

---

*This document is updated regularly. Last updated: [Date of creation]* 