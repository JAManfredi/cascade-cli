# Homebrew Formula for Cascade CLI
# To install: brew install JAManfredi/cascade-cli/cascade-cli

class CascadeCli < Formula
  desc "Stacked diffs CLI for Bitbucket Server"
  homepage "https://github.com/JAManfredi/cascade-cli"
  url "https://github.com/JAManfredi/cascade-cli/archive/v0.1.0.tar.gz"
  # sha256 "TODO: Update this after creating the v0.1.0 release tag"
  # To get the hash: curl -L https://github.com/JAManfredi/cascade-cli/archive/v0.1.0.tar.gz | sha256sum
  license "MIT OR Apache-2.0"
  head "https://github.com/JAManfredi/cascade-cli.git", branch: "master"

  depends_on "rust" => :build
  depends_on "git"

  def install
    system "cargo", "install", *std_cargo_args
    
    # Install shell completions
    bash_completion.install "completions/cc.bash" => "cc"
    zsh_completion.install "completions/_cc"
    fish_completion.install "completions/cc.fish"
    
    # Install man page if available
    # man1.install "docs/cc.1" if File.exist?("docs/cc.1")
  end

  def post_install
    puts <<~EOS
      Cascade CLI has been installed!
      
      To get started:
        1. Navigate to your Git repository: cd your-project
        2. Run the setup wizard: cc setup
        3. Create your first stack: cc stack create "my-feature"
      
      For documentation and examples:
        cc --help
        https://github.com/JAManfredi/cascade-cli
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