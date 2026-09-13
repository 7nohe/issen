#!/usr/bin/env node
// Assemble the npm packages from a release's archives:
//
//   node npm/build.mjs --version 0.1.0 --dist <directory of archives> --out <directory>
//
// Writes <out>/issen and one directory per platform package, each ready for
// `npm publish`. The archives are the ones the release workflow uploads, so npm
// ships byte-for-byte the binaries that were smoke-tested there.

import { execFileSync } from "node:child_process";
import { chmodSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
const { PLATFORMS } = createRequire(import.meta.url)("./issen/index.js");

const { values } = parseArgs({
  options: { version: { type: "string" }, dist: { type: "string" }, out: { type: "string" } },
});
if (!values.version || !values.dist || !values.out) {
  console.error("usage: node npm/build.mjs --version <x.y.z> --dist <archives> --out <directory>");
  process.exit(2);
}
const version = values.version.replace(/^v/, "");
const main = JSON.parse(readFileSync(join(here, "issen", "package.json"), "utf8"));
const repository = { type: main.repository.type, url: main.repository.url };

rmSync(values.out, { recursive: true, force: true });
mkdirSync(values.out, { recursive: true });

for (const p of PLATFORMS) {
  const stem = `issen-v${version}-${p.target}`;
  const archive = join(values.dist, `${stem}.${p.os === "win32" ? "zip" : "tar.gz"}`);
  if (!existsSync(archive)) throw new Error(`missing archive: ${archive}`);

  const unpacked = mkdtempSync(join(tmpdir(), "issen-npm-"));
  if (archive.endsWith(".zip")) execFileSync("unzip", ["-q", archive, "-d", unpacked]);
  else execFileSync("tar", ["-xzf", archive, "-C", unpacked]);

  const dir = join(values.out, p.package.split("/")[1]);
  mkdirSync(dir);
  for (const file of [p.binary, "LICENSE", "NOTICE"]) cpSync(join(unpacked, stem, file), join(dir, file));
  if (p.os !== "win32") chmodSync(join(dir, p.binary), 0o755);
  rmSync(unpacked, { recursive: true, force: true });

  const manifest = {
    name: p.package,
    version,
    description: `The ${p.target} binary for issen. Install the issen package instead.`,
    homepage: main.homepage,
    repository,
    license: main.license,
    os: [p.os],
    cpu: [p.cpu],
    ...(p.libc ? { libc: [p.libc] } : {}),
    files: [p.binary, "LICENSE", "NOTICE"],
  };
  writeFileSync(join(dir, "package.json"), `${JSON.stringify(manifest, null, 2)}\n`);
}

const mainDir = join(values.out, "issen");
cpSync(join(here, "issen"), mainDir, { recursive: true });
for (const file of ["LICENSE", "NOTICE"]) cpSync(join(root, file), join(mainDir, file));
main.version = version;
main.optionalDependencies = Object.fromEntries(PLATFORMS.map((p) => [p.package, version]));
writeFileSync(join(mainDir, "package.json"), `${JSON.stringify(main, null, 2)}\n`);

console.log(`assembled issen ${version} and ${PLATFORMS.length} platform packages in ${values.out}`);
