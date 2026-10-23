// SPDX-License-Identifier: MPL-2.0

// Commit the bundle with source changes so CI needs no package installation.
const fs = require('node:fs');
const path = require('node:path');
const esbuild = require('esbuild');

const bundle = esbuild.buildSync({
  absWorkingDir: __dirname,
  entryPoints: [path.join(__dirname, 'src/index.cjs')],
  outfile: path.join(__dirname, 'runtime/dist/index.js'),
  bundle: true, platform: 'node', target: 'node24', legalComments: 'inline', write: false,
});

const lock = require('./package-lock.json');
const licenses = [];
for (const [location, metadata] of Object.entries(lock.packages)) {
  if (!location || metadata.dev) continue;
  const directory = path.join(__dirname, location);
  const license = fs.readdirSync(directory).find(file => /^licen[cs]e(?:\.|$)/i.test(file));
  if (!license) throw new Error(`missing license for ${location}`);
  licenses.push(`${location} (${metadata.version})\n\n${fs.readFileSync(path.join(directory, license), 'utf8')}`);
}
const outputs = [...bundle.outputFiles, {
  path: path.join(__dirname, 'runtime/dist/LICENSES.txt'), contents: Buffer.from(licenses.join('\n\n') + '\n'),
}];
for (const output of outputs) {
  if (process.argv.includes('--check')) {
    if (!fs.existsSync(output.path) || !fs.readFileSync(output.path).equals(Buffer.from(output.contents))) {
      throw new Error('bundled runtime is stale; run npm run build');
    }
  } else {
    fs.mkdirSync(path.dirname(output.path), { recursive: true });
    fs.writeFileSync(output.path, output.contents);
  }
}
