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
    url "https://github.com/JAManfredi/cascade-cli/releases/download/v0.1.6/csc-macos-arm64.tar.gz"
    sha256 "TBD_ARM64_SHA"
    version "0.1.6"
  else
    # Intel (x64)
    url "https://github.com/JAManfredi/cascade-cli/releases/download/v0.1.6/csc-macos-x64.tar.gz"
    sha256 "TBD_X64_SHA"
    version "0.1.6"
  end

  depends_on "git"

  def install
    bin.install "csc"
    
    # Install shell completions
    bash_completion.install "completions/csc.bash" => "csc"
    zsh_completion.install "completions/_csc"
    fish_completion.install "completions/csc.fish"
  end

  def post_install
    puts <<~EOS
      Cascade CLI has been installed!
      
      Quick Start:
        1. Navigate to your Git repository: cd your-project
        2. Initialize Cascade: csc init
        3. Create your first stack: csc stack create "my-feature"
        4. Add commits to stack: csc stack push
      
      Learn More:
        csc --help                    # Show all commands
        csc doctor                    # Check system setup
        csc stack --help             # Stack management help
        
      Documentation:
        https://github.com/JAManfredi/cascade-cli/blob/main/docs/USER_MANUAL.md
        https://github.com/JAManfredi/cascade-cli/blob/main/docs/ONBOARDING.md
      
      Shell Completions:
        Completions have been installed for Bash, Zsh, and Fish.
        Restart your shell or source your profile to enable them.
    EOS
  end

  test do
    system "#{bin}/csc", "--version"
    system "#{bin}/csc", "--help"
    
    # Test basic functionality
    (testpath/"test-repo").mkpath
    cd testpath/"test-repo" do
      system "git", "init"
      system "git", "config", "user.name", "Test User"
      system "git", "config", "user.email", "test@example.com"
      system "echo 'test' > README.md"
      system "git", "add", "README.md"
      system "git", "commit", "-m", "Initial commit"
      
      # Test csc doctor (should detect git repo but no cascade config)
      output = shell_output("#{bin}/csc doctor 2>&1", 1)
      assert_match "Git repository:", output
    end
  end
end 