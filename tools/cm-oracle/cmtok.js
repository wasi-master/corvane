// GHD highlighter oracle: node cmtok.js <mime> <mode-module> <file>
// Prints one JSON line per token: [line, start, length, token] exactly as
// GitHub Desktop's highlighter worker (app/src/highlighter/index.ts) computes them.
const path = require('path');
const ROOT = process.env.CM_ORACLE_DIR || path.join(__dirname, '..', '..', 'target', 'cm-oracle');
const fs = require('fs');
const CM = require(path.join(ROOT, 'node_modules/codemirror/addon/runmode/runmode.node.js'));
const [mime, modeModule, file] = process.argv.slice(2);
require(modeModule.startsWith('codemirror-mode') ? path.join(ROOT, 'node_modules', modeModule) : path.join(ROOT, 'node_modules/codemirror/mode', modeModule));
const mode = CM.getMode({}, mime);
const lines = fs.readFileSync(file, 'utf8').split(/\r?\n|\r/);
const state = mode.startState ? mode.startState() : null;
const name = m => (m && typeof m.name === 'string') ? m.name : null;
const inner = st => { const i = CM.innerMode(mode, st); return i && i.mode ? name(i.mode) : null; };
const out = [];
lines.forEach((line, ix) => {
  if (!line.length) { if (mode.blankLine) mode.blankLine(state); return; }
  const stream = new CM.StringStream(line, 4, { lines, line: ix, lookAhead: n => lines[ix + n] });
  while (!stream.eol()) {
    let token = null, ok = false;
    for (let i = 0; i < 10; i++) {
      const im = inner(state);
      const t = mode.token(stream, state);
      if (stream.pos > stream.start) { token = t && im ? `m-${im} ${t}` : t; ok = true; break; }
    }
    if (!ok) throw new Error('failed to advance');
    if (token) out.push([ix, stream.start, stream.pos - stream.start, token]);
    stream.start = stream.pos;
  }
});
process.stdout.write(out.map(t => JSON.stringify(t)).join('\n') + '\n');
