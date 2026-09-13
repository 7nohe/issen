#!/usr/bin/env bash
# Print the Homebrew formula for a release:
#
#   packaging/homebrew/formula.sh <version> <SHA256SUMS> [base URL]
#
# The base URL defaults to the release's download location on GitHub. The
# formula installs the prebuilt archives the release workflow smoke-tested;
# Linux takes the glibc build, which Homebrew on Linux always satisfies.
set -euo pipefail

if [ $# -lt 2 ]; then
  echo "usage: $0 <version> <SHA256SUMS> [base URL]" >&2
  exit 2
fi
version="${1#v}"
sums="$2"
base="${3:-https://github.com/7nohe/issen/releases/download/v$version}"

sha() {
  local file="issen-v$version-$1.tar.gz"
  # sha256sum marks binary-mode lines with "*" before the name.
  awk -v f="$file" '{ name = $2; sub(/^\*/, "", name) } name == f { print $1; found = 1 } END { exit !found }' "$sums" ||
    { echo "no checksum for $file in $sums" >&2; return 1; }
}

# Looked up before the heredoc on purpose: a failing command substitution inside
# a heredoc does not stop the script, and would print a formula with empty
# checksums and exit 0.
mac_arm=$(sha aarch64-apple-darwin)
mac_intel=$(sha x86_64-apple-darwin)
linux_arm=$(sha aarch64-unknown-linux-gnu)
linux_intel=$(sha x86_64-unknown-linux-gnu)

cat <<RUBY
class Issen < Formula
  desc "Fast, deterministic linter for Japanese Markdown prose"
  homepage "https://github.com/7nohe/issen"
  license "MIT"

  on_macos do
    on_arm do
      url "$base/issen-v$version-aarch64-apple-darwin.tar.gz"
      sha256 "$(sha aarch64-apple-darwin)"
    end
    on_intel do
      url "$base/issen-v$version-x86_64-apple-darwin.tar.gz"
      sha256 "$(sha x86_64-apple-darwin)"
    end
  end

  on_linux do
    on_arm do
      url "$base/issen-v$version-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "$(sha aarch64-unknown-linux-gnu)"
    end
    on_intel do
      url "$base/issen-v$version-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "$(sha x86_64-unknown-linux-gnu)"
    end
  end

  def install
    bin.install "issen"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/issen --version")
    (testpath/"doc.md").write "ﾃｽﾄです。\n"
    assert_match "no-hankaku-kana", shell_output("#{bin}/issen #{testpath}/doc.md", 1)
  end
end
RUBY
