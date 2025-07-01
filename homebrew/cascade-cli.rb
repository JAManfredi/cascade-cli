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
  desc "Cascade CLI - Git stacked diffs for Bitbucket Server"
  homepage "https://github.com/JAManfredi/cascade-cli"
  version "0.1.16"
  license "MIT"

  # macOS binaries with architecture detection
  if Hardware::CPU.arm?
    url "https://github.com/JAManfredi/cascade-cli/releases/download/v0.1.16/ca-macos-arm64.tar.gz"
    sha256 "7d1597bc69d9e525c40d1921731f71f924428516a11df244b569d3f5edeb8d78"
  else
    url "https://github.com/JAManfredi/cascade-cli/releases/download/v0.1.16/ca-macos-x64.tar.gz"
    sha256 "e173dd409ac2b9c8066fac3046b097ceef78b5aceeed88162ab8fa7b9049fecf"
  end

  depends_on "git"

  def install
    bin.install "ca"
    
    # Install shell completions
    bash_completion.install "completions/ca.bash" => "ca"
    zsh_completion.install "completions/_ca"
    fish_completion.install "completions/ca.fish"
  end

  def caveats
    <<~EOS
      Cascade CLI has been installed successfully!
      
      Getting Started:
      1. Navigate to your Git repository
      2. Initialize Cascade: ca init
      3. Create your first stack: ca stack create "my-feature"
      4. Add commits to stack: ca stack push
      
      Quick Commands:
      ca --help                    # Show all commands
      ca doctor                    # Check system setup
      ca stack --help             # Stack management help
      
      Documentation: https://github.com/JAManfredi/cascade-cli/blob/main/docs/
    EOS
  end

  test do
    # Test basic functionality
    system "#{bin}/ca", "--version"
    system "#{bin}/ca", "--help"
    
    # Test in a temporary directory (no git repo)
    testpath_git = testpath/"test_repo"
    testpath_git.mkpath
    
    cd testpath_git do
      system "git", "init"
      system "git", "config", "user.name", "Test User"
      system "git", "config", "user.email", "test@example.com"
      
      # Test ca doctor (should detect git repo but no cascade config)
      output = shell_output("#{bin}/ca doctor 2>&1", 1)
      assert_match "Cascade CLI", output
    end
  end
end 