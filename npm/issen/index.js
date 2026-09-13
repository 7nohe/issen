"use strict";

const fs = require("node:fs");

// Release target -> npm platform package. build.mjs reads this table too, so the
// packages it assembles and the ones resolved here cannot drift apart.
const PLATFORMS = [
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

// The host C library on Linux. Reading ldd is a file read; the process report
// is the fallback because generating it costs more than the launch it serves.
function hostLibc() {
  if (process.platform !== "linux") return undefined;
  try {
    const ldd = fs.readFileSync("/usr/bin/ldd", "utf8");
    if (ldd.includes("musl")) return "musl";
    if (ldd.includes("GLIBC") || ldd.includes("GNU C Library")) return "glibc";
  } catch {
    // No ldd script to read; ask the runtime instead.
  }
  const header = process.report && process.report.getReport().header;
  return header && header.glibcVersionRuntime ? "glibc" : "musl";
}

/**
 * Absolute path of the issen binary for this platform.
 *
 * Package managers install only the Linux variant whose `libc` matches the
 * host, but one that ignores the field, or a forced install, can leave both in
 * place, so the variant matching the host wins. A glibc host falls back to the
 * static musl build; a musl host never falls back to the glibc one, which could
 * not start.
 */
function binaryPath() {
  if (process.env.ISSEN_BINARY) return process.env.ISSEN_BINARY;
  const libc = hostLibc();
  const candidates = PLATFORMS.filter(
    (p) => p.os === process.platform && p.cpu === process.arch && (libc !== "musl" || p.libc === "musl"),
  ).sort((a, b) => (b.libc === libc) - (a.libc === libc));
  for (const p of candidates) {
    try {
      return require.resolve(`${p.package}/${p.binary}`);
    } catch {
      // Not installed; try the next variant.
    }
  }
  if (candidates.length === 0) {
    throw new Error(`issen: no prebuilt binary for ${process.platform}-${process.arch}${libc ? ` (${libc})` : ""}.`);
  }
  throw new Error(
    `issen: ${candidates[0].package} is not installed. It is an optional dependency; reinstall without --omit=optional or --no-optional.`,
  );
}

module.exports = { binaryPath, PLATFORMS };
