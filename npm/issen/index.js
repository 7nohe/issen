"use strict";

const fs = require("node:fs");
const PLATFORMS = require("./platforms.js");

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
  return process.report.getReport().header.glibcVersionRuntime ? "glibc" : "musl";
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
  const candidates = (libc === "glibc" ? ["glibc", "musl"] : [libc]).flatMap((l) =>
    PLATFORMS.filter((p) => p.os === process.platform && p.cpu === process.arch && p.libc === l),
  );
  if (candidates.length === 0) {
    throw new Error(`issen: no prebuilt binary for ${process.platform}-${process.arch}${libc ? ` (${libc})` : ""}.`);
  }
  for (const p of candidates) {
    try {
      return require.resolve(`${p.package}/${p.binary}`);
    } catch {
      // Not installed; try the next variant.
    }
  }
  throw new Error(
    `issen: ${candidates[0].package} is not installed. It is an optional dependency; reinstall without --omit=optional or --no-optional.`,
  );
}

module.exports = { binaryPath };
