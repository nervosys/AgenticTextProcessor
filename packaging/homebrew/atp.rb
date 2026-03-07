class Atp < Formula
  desc "Agentic Text Processor — an agentic-first successor to grep, sed, and awk"
  homepage "https://github.com/nervosys/AgenticTextProcessor"
  version "1.1.0"
  license "AGPL-3.0-only"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/nervosys/AgenticTextProcessor/releases/download/v#{version}/atp-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_SHA256_AARCH64_DARWIN"
    else
      url "https://github.com/nervosys/AgenticTextProcessor/releases/download/v#{version}/atp-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_SHA256_X86_64_DARWIN"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/nervosys/AgenticTextProcessor/releases/download/v#{version}/atp-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_SHA256_AARCH64_LINUX"
    else
      url "https://github.com/nervosys/AgenticTextProcessor/releases/download/v#{version}/atp-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_SHA256_X86_64_LINUX"
    end
  end

  def install
    bin.install "atp"
    bin.install "atp-grep" if File.exist?("atp-grep")
    bin.install "atp-sed" if File.exist?("atp-sed")
    bin.install "atp-awk" if File.exist?("atp-awk")

    # Install shell completions if present
    bash_completion.install "completions/atp.bash" if File.exist?("completions/atp.bash")
    zsh_completion.install "completions/_atp" if File.exist?("completions/_atp")
    fish_completion.install "completions/atp.fish" if File.exist?("completions/atp.fish")

    # Install man page if present
    man1.install "man/atp.1" if File.exist?("man/atp.1")
  end

  test do
    assert_match "atp", shell_output("#{bin}/atp --version")

    # Basic grep test
    (testpath/"test.txt").write("hello world\nfoo bar\nhello again\n")
    output = shell_output("#{bin}/atp search 'hello' #{testpath}/test.txt --format json")
    assert_match "hello", output

    # Basic AQL test
    output = shell_output("echo 'hello world' | #{bin}/atp query 'find \"hello\"'")
    assert_match "hello", output
  end
end
