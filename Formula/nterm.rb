class Nterm < Formula
  desc "A terminal-based IDE with file tree, editor, terminal, and AI chat"
  homepage "https://github.com/ashishtyagi10/nterm"
  url "https://github.com/ashishtyagi10/nterm/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "REPLACE_WITH_SHA256_AFTER_RELEASE"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    # nterm requires a TTY, so just check it exists
    assert_predicate bin/"nterm", :exist?
  end
end
