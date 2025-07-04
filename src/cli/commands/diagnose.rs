use crate::errors::{CascadeError, Result};
use crate::git::GitRepository;
use std::path::Path;

/// Run diagnostic checks for git2 TLS/SSH support
pub async fn run() -> Result<()> {
    println!("🔧 Cascade CLI - Git2 Diagnostics\n");
    
    // Check git2 features
    let features = git2::features();
    println!("🔍 Git2 Feature Support:");
    println!("  HTTPS/TLS support: {}", if features.https() { "✅ YES" } else { "❌ NO" });
    println!("  SSH support: {}", if features.ssh() { "✅ YES" } else { "❌ NO" });
    
    // Get libgit2 version
    if let Ok(version) = git2::version() {
        println!("  libgit2 version: {}.{}.{}", version.0, version.1, version.2);
    }
    
    println!();
    
    // Check current repository if we're in one
    if let Ok(repo) = GitRepository::open(Path::new(".")) {
        println!("📁 Current Repository Analysis:");
        
        // Use the built-in diagnostic method
        repo.diagnose_git2_support()?;
        
        // Check remote URLs
        if let Ok(remote_url) = repo.get_remote_url("origin") {
            println!("\n🌐 Remote Configuration:");
            println!("  Origin URL: {}", remote_url);
            
            if remote_url.starts_with("https://") {
                if features.https() {
                    println!("  ✅ HTTPS remote with TLS support - should work!");
                } else {
                    println!("  ❌ HTTPS remote but NO TLS support - will fallback to git CLI");
                }
            } else if remote_url.starts_with("git@") || remote_url.starts_with("ssh://") {
                if features.ssh() {
                    println!("  ✅ SSH remote with SSH support - should work!");
                } else {
                    println!("  ❌ SSH remote but NO SSH support - will fallback to git CLI");
                }
            }
        }
    } else {
        println!("📁 Not in a git repository - skipping repository-specific checks");
    }
    
    // Provide recommendations
    println!("\n💡 Recommendations:");
    
    if !features.https() || !features.ssh() {
        println!("  🔧 MISSING FEATURES DETECTED:");
        println!("     Your git2 is missing TLS/SSH support.");
        println!("     This causes performance issues due to git CLI fallbacks.");
        println!();
        println!("  📝 TO FIX: Update Cargo.toml git2 dependency:");
        println!("     git2 = {{ version = \"0.20.2\", features = [\"vendored-libgit2\", \"https\", \"ssh\"] }}");
        println!();
        println!("  🚀 BENEFITS: Direct git2 operations (faster, more reliable)");
    } else {
        println!("  ✅ git2 has full TLS/SSH support!");
        println!("     If you're still experiencing issues, they may be:");
        println!("     - Network connectivity problems");
        println!("     - Authentication/credential issues");
        println!("     - Corporate firewall/proxy settings");
        println!("     - SSL certificate verification problems");
    }
    
    Ok(())
}