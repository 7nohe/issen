# issen

A fast, deterministic linter for Japanese Markdown prose, built as a harness for documents written by AI agents. This package installs the prebuilt `issen` binary for your platform.

```bash
npm install --save-dev issen
npx issen README.md
cat generated.md | npx issen --stdin --fix --format json
```

Usage, configuration, and the rules are documented in the [issen repository](https://github.com/7nohe/issen). How closely it follows `textlint-rule-preset-ja-technical-writing` is measured in [COMPATIBILITY.md](https://github.com/7nohe/issen/blob/main/COMPATIBILITY.md).

## Calling it in a loop

The `issen` command is a small Node.js script that starts the binary, so every run pays for Node.js start-up as well. On a single document that is most of the time. Measured on an Apple silicon Mac, one lint of a 12 KB document takes about 7 ms from the binary, 37 ms through the installed `issen` command, and 250 ms through `npx issen`. An agent that lints each document it generates should spawn the binary directly:

```js
const { spawnSync } = require("node:child_process");
const { binaryPath } = require("issen");

const result = spawnSync(binaryPath(), ["--stdin", "--format", "json"], { input: markdown });
const report = JSON.parse(result.stdout);
```

Set `ISSEN_BINARY` to use a binary from elsewhere.

## Platforms

macOS (Apple silicon, Intel), Linux (x64, arm64), and Windows (x64). On Linux there is a glibc build, which needs glibc 2.34 or later, and a static musl build for Alpine and other systems without glibc. The package manager installs the one matching the host through the `libc` field; if an installer puts both in place, the launcher still picks the right one.

## License

MIT. The binary embeds the IPADIC dictionary, whose notice is in `NOTICE` and has to accompany any redistribution.
