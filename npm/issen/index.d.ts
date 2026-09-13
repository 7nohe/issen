/**
 * Absolute path of the issen binary for this platform. Spawn it directly when
 * calling issen repeatedly, to skip the Node.js start-up the `issen` command
 * pays on every run. `ISSEN_BINARY` overrides the lookup.
 */
export declare function binaryPath(): string;
