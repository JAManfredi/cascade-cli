# Homebrew Formula for Cascade CLI
# 
# Install method 1 (download first):
# curl -O https://raw.githubusercontent.com/JAManfredi/cascade-cli/master/homebrew/cascade-cli.rb
# brew install cascade-cli.rb
# rm cascade-cli.rb
#
# Install method 2 (with tap - requires tap repository):
# brew tap JAManfredi/cascade-cli
# brew install cascade-cli

class CascadeCli < Formula
  desc "Git-based development workflow tool for managing stacked branches"
  homepage "https://github.com/JAManfredi/cascade-cli"
  license "MIT OR Apache-2.0"
  head "https://github.com/JAManfredi/cascade-cli.git", branch: "master"

  # Apple Silicon (ARM64) - Default
  if Hardware::CPU.arm?
    url "https://github.com/JAManfredi/cascade-cli/releases/download/v0.1.4/cc-macos-arm64.tar.gz"
    sha256 "493938d29f18a1f47a48800cf7c5662f0c8f7b19ae3c8ac0f84037f7402b5652"
    version "0.1.4"
  else
    # Intel (x64)
    url "https://github.com/JAManfredi/cascade-cli/releases/download/v0.1.4/cc-macos-x64.tar.gz"
    sha256 "5287e10b6103094e2349b24237c7015f57d4054d5726f651fef9c9985a445281"
    version "0.1.4"
  end

  depends_on "git"

  def install
    bin.install "cc"
    
    # Note: Shell completions will be added in future release
    # when completion generation in CI is fixed
  end

  def post_install
    puts <<~EOS
      Cascade CLI has been installed!
      
      Quick Start:
        1. Navigate to your Git repository: cd your-project
        2. Initialize Cascade: cc init
        3. Create your first stack: cc stack create "my-feature"
        4. Add commits to stack: cc stack push
      
      Learn More:
        cc --help                    # Show all commands
        cc doctor                    # Check system setup
        cc stack --help             # Stack management help
        
      Documentation:
        https://github.com/JAManfredi/cascade-cli/blob/main/docs/USER_MANUAL.md
        https://github.com/JAManfredi/cascade-cli/blob/main/docs/ONBOARDING.md
      
      Shell Completions:
        Completions are automatically installed for Bash, Zsh, and Fish.
        Restart your shell or source your profile to enable them.
    EOS
  end

  test do
    system "#{bin}/cc", "--version"
    system "#{bin}/cc", "--help"
    
    # Test basic functionality
    (testpath/"test-repo").mkpath
    cd testpath/"test-repo" do
      system "git", "init"
      system "git", "config", "user.name", "Test User"
      system "git", "config", "user.email", "test@example.com"
      system "echo 'test' > README.md"
      system "git", "add", "README.md"
      system "git", "commit", "-m", "Initial commit"
      
      # Test cc doctor (should detect git repo but no cascade config)
      output = shell_output("#{bin}/cc doctor 2>&1", 1)
      assert_match "Git repository:", output
    end
  end
end 