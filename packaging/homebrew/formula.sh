#!/usr/bin/env bash
# Print the Homebrew formula for a release:
#
#   packaging/homebrew/formula.sh <version> <SHA256SUMS> [base URL]
#
# The base URL defaults to the release's download location on GitHub. The
# formula installs the prebuilt archives the release workflow smoke-tested.
# Linux takes the static musl build: the glibc build needs glibc 2.34, and
# Homebrew leaves a prebuilt binary on the host's own glibc, which may be older.
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
  awk -v f="$file" '$2 == f { print $1; found = 1 } END { exit !found }' "$sums" ||
    { echo "no checksum for $file in $sums" >&2; return 1; }
}

# Looked up before the heredoc on purpose: a failing command substitution inside
# a heredoc does not stop the script, and would print a formula with empty
# checksums and exit 0.
mac_arm=$(sha aarch64-apple-darwin)
mac_intel=$(sha x86_64-apple-darwin)
linux_arm=$(sha aarch64-unknown-linux-musl)
linux_intel=$(sha x86_64-unknown-linux-musl)

cat <<RUBY
class Issen < Formula
  desc "Fast, deterministic linter for Japanese Markdown prose"
  homepage "https://github.com/7nohe/issen"
  license "MIT"

  on_macos do
    on_arm do
      url "$base/issen-v$version-aarch64-apple-darwin.tar.gz"
      sha256 "$mac_arm"
    end
    on_intel do
      url "$base/issen-v$version-x86_64-apple-darwin.tar.gz"
      sha256 "$mac_intel"
    end
  end

  on_linux do
    on_arm do
      url "$base/issen-v$version-aarch64-unknown-linux-musl.tar.gz"
      sha256 "$linux_arm"
    end
    on_intel do
      url "$base/issen-v$version-x86_64-unknown-linux-musl.tar.gz"
      sha256 "$linux_intel"
    end
  end

  def install
    bin.install "issen"
    # LICENSE and NOTICE are installed by name; this one has to be asked for.
    prefix.install "THIRD-PARTY-LICENSES"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/issen --version")
    (testpath/"doc.md").write "ﾃｽﾄです。\n"
    assert_match "no-hankaku-kana", shell_output("#{bin}/issen #{testpath}/doc.md", 1)
  end
end
RUBY
