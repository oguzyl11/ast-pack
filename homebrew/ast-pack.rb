class AstPack < Formula
  desc "Fold your codebase into an LLM token budget. Unfold on demand."
  homepage "https://github.com/oguzyl11/ast-pack"
  license "MIT OR Apache-2.0"
  head "https://github.com/oguzyl11/ast-pack.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "ast-pack", shell_output("#{bin}/ast-pack --version")
  end
end
