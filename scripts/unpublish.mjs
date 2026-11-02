import './env.mjs';
import { neon } from '@neondatabase/serverless';
import { del } from '@vercel/blob';

function parseArgs(argv) {
  let slug = null;
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--slug') slug = argv[++i];
    else if (!argv[i].startsWith('--')) slug = argv[i];
  }
  return { slug };
}

async function main() {
  const { slug } = parseArgs(process.argv.slice(2));
  if (!slug) {
    console.error('Usage: unpublish.mjs --slug <slug>');
    process.exit(1);
  }
  const token = process.env.BLOB_READ_WRITE_TOKEN;
  const dbUrl = process.env.DATABASE_URL || process.env.POSTGRES_URL || process.env.NEON_DATABASE_URL;
  if (!token) {
    console.error('BLOB_READ_WRITE_TOKEN is required');
    process.exit(1);
  }
  if (!dbUrl) {
    console.error('DATABASE_URL is required');
    process.exit(1);
  }
  const sql = neon(dbUrl);
  const rows = await sql('SELECT stream_url FROM videos WHERE slug = $1', [slug]);
  if (!rows || rows.length === 0) {
    console.log('No video found for slug:', slug, '(nothing to do)');
    process.exit(0);
  }
  const url = rows[0].stream_url;
  try {
    await del(url, { token });
    console.log('Deleted blob.');
  } catch (err) {
    console.warn('Blob delete failed:', err && err.message ? err.message : String(err));
  }
  await sql('DELETE FROM videos WHERE slug = $1', [slug]);
  console.log('Unpublished:', slug);
}

main().catch((err) => {
  console.error('unpublish failed:', err && err.message ? err.message : String(err));
  process.exit(1);
});