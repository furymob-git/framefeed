import { readdirSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const dir = import.meta.dirname;
for (const f of readdirSync(dir)) {
  if (f === 'index.js') continue;
  if (!f.endsWith('.mjs') && !f.endsWith('.js')) continue;
  await import(pathToFileURL(path.join(dir, f)).href);
}