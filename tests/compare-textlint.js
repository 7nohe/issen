// Compare two textlint-format JSON outputs: node compare-tl.js <reference.json> <candidate.json>
const fs = require("fs");
const [refPath, candPath] = process.argv.slice(2);
const load = p => JSON.parse(fs.readFileSync(p, "utf8")).flatMap(f => f.messages.map(m => ({ rule: m.ruleId, line: m.line, col: m.column, msg: m.message.split("\n")[0] })));
const ref = load(refPath), cand = load(candPath);
const kx = d => `${d.rule}@${d.line}:${d.col}`, kl = d => `${d.rule}@${d.line}`;
const candX = new Set(cand.map(kx)), candL = new Map(cand.map(d => [kl(d), d])), refL = new Set(ref.map(kl));
let exact = 0, lineOnly = [], missing = [];
for (const d of ref) { if (candX.has(kx(d))) exact++; else if (candL.has(kl(d))) lineOnly.push([d, candL.get(kl(d))]); else missing.push(d); }
const extra = cand.filter(d => !refL.has(kl(d)));
console.log(`reference: ${ref.length}  exact(rule+line+col): ${exact}  line-only: ${lineOnly.length}  missing: ${missing.length}  extra: ${extra.length}`);
for (const [d, c] of lineOnly) console.log(`  ~ ${d.rule} line ${d.line}: ref col ${d.col} / cand col ${c.col}`);
for (const d of missing) console.log(`  - ${d.rule} ${d.line}:${d.col} ${d.msg}`);
for (const d of extra) console.log(`  + ${d.rule} ${d.line}:${d.col} ${d.msg}`);
