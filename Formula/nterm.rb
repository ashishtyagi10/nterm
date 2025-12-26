class Nterm < Formula
  desc "A terminal-based IDE with file tree, editor, terminal, and AI chat"
  homepage "https://github.com/ashishtyagi10/nterm"
  url "https://github.com/ashishtyagi10/nterm/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "ed2ef4fa7563a2d7d7ea5e47e8b90fb13a2d00580faf004dbb7d59936115ccd7"
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
