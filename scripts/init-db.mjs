import './env.mjs';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { neon } from '@neondatabase/serverless';

const __dirname = dirname(fileURLToPath(import.meta.url));

async function main() {
  const url = process.env.DATABASE_URL || process.env.POSTGRES_URL || process.env.NEON_DATABASE_URL;
  if (!url) {
    console.error('DATABASE_URL is required');
    process.exit(1);
  }
  const sqlText = readFileSync(join(__dirname, 'schema.sql'), 'utf8');
  const sql = neon(url);
  const statements = sqlText
    .split(/;\s*\n/)
    .map((s) => s.trim())
    .filter(Boolean)
    .map((s) => s + ';');
  for (const stmt of statements) {
    await sql(stmt);
  }
  console.log('Database initialized.');
}

main().catch((err) => {
  console.error('init-db failed:', err && err.message ? err.message : String(err));
  process.exit(1);
});