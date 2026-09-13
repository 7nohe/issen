"use strict";

// Release target -> npm platform package. build.mjs reads this table too, so the
// packages it assembles and the ones resolved at run time cannot drift apart.
module.exports = [
  { target: "aarch64-apple-darwin", os: "darwin", cpu: "arm64" },
  { target: "x86_64-apple-darwin", os: "darwin", cpu: "x64" },
  { target: "aarch64-unknown-linux-gnu", os: "linux", cpu: "arm64", libc: "glibc" },
  { target: "x86_64-unknown-linux-gnu", os: "linux", cpu: "x64", libc: "glibc" },
  { target: "aarch64-unknown-linux-musl", os: "linux", cpu: "arm64", libc: "musl" },
  { target: "x86_64-unknown-linux-musl", os: "linux", cpu: "x64", libc: "musl" },
  { target: "x86_64-pc-windows-msvc", os: "win32", cpu: "x64" },
].map((p) => ({
  ...p,
  package: `@issen/cli-${p.os}-${p.cpu}${p.libc === "musl" ? "-musl" : ""}`,
  binary: p.os === "win32" ? "issen.exe" : "issen",
}));
