class Floo < Formula
  desc "Sticky port assignments for parallel coding-agent dev servers"
  homepage "https://gitlab.com/ajlebaron/floo"
  url "https://static.crates.io/crates/floo-ports/floo-ports-0.0.2.crate"
  sha256 "984974d9030252b7286eace516e6ed8ae35e9f3605f60147b42b5b0dde3a9274"
  license "MIT"

  depends_on "rust" => :build

  def install
    # The crates.io tarball unpacks to floo-ports-<version>/. rusqlite is built
    # with the bundled feature, so SQLite is compiled from source and there is
    # no system libsqlite3 dependency. The package name is floo-ports but the
    # binary it installs is named floo.
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "floo #{version}", shell_output("#{bin}/floo version")
  end
end
