class Termula < Formula
  desc "Render LaTeX math beautifully in your terminal"
  homepage "https://github.com/nicokeywords/termula"
  license "MIT"

  # Update these for each release
  version "0.1.0"
  url "https://github.com/nicokeywords/termula/archive/refs/tags/v#{version}.tar.gz"
  # sha256 "UPDATE_WITH_ACTUAL_SHA256"

  depends_on "rust" => :build
  depends_on "utftex" => :recommended

  def install
    system "cargo", "install", *std_cargo_args
    # Generate shell completions
    bash_completion.install Utils.safe_popen_read(bin/"termula", "--completions", "bash").to_s => "termula"
    zsh_completion.install Utils.safe_popen_read(bin/"termula", "--completions", "zsh").to_s => "_termula"
    fish_completion.install Utils.safe_popen_read(bin/"termula", "--completions", "fish").to_s => "termula.fish"
  end

  test do
    # Basic pipe mode test
    output = pipe_output("#{bin}/termula -m inline", "$$\\alpha$$")
    assert_match "α", output
  end
end
