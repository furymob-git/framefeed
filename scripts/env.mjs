// Load `.env.local` for local CLI scripts, if it exists.
//
// Vercel injects environment variables into functions at runtime; this file is
// only imported by the publish/unpublish/db-init scripts. Keeping the load here
// means `npm run publish -- --dry-run` works before any credentials exist.
import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const envPath = path.join(root, '.env.local');

if (existsSync(envPath)) {
  try {
    process.loadEnvFile(envPath);
  } catch (err) {
    console.warn('Could not read .env.local:', err && err.message ? err.message : String(err));
  }
}
