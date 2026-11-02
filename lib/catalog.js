import { neon } from '@neondatabase/serverless';

let client = null;
let initialized = false;

function getClient() {
  if (initialized) return client;
  initialized = true;
  const url = process.env.DATABASE_URL || process.env.POSTGRES_URL || process.env.NEON_DATABASE_URL;
  if (!url) return null;
  try {
    client = neon(url);
  } catch {
    client = null;
  }
  return client;
}

export async function listVideos() {
  const sql = getClient();
  if (!sql) throw new Error('catalog unavailable');
  const rows = await sql(
    'SELECT slug, title, description, cols, rows, fps, format, width_px, height_px, duration_seconds, size_bytes, created_at FROM videos WHERE published = true ORDER BY created_at DESC'
  );
  return rows.map((r) => ({
    slug: r.slug,
    title: r.title,
    description: r.description,
    cols: r.cols,
    rows: r.rows,
    fps: r.fps,
    format: r.format || 'ansi',
    widthPx: r.width_px,
    heightPx: r.height_px,
    durationSeconds: Number(r.duration_seconds),
    sizeBytes: Number(r.size_bytes),
    createdAt: r.created_at,
  }));
}

export async function getVideo(slug) {
  const sql = getClient();
  if (!sql) throw new Error('catalog unavailable');
  const rows = await sql(
    'SELECT slug, title, stream_url, width_px, height_px, fps, frame_count, duration_seconds, format FROM videos WHERE slug = $1 AND published = true LIMIT 1',
    [slug]
  );
  if (!rows || rows.length === 0) return null;
  const r = rows[0];
  return {
    slug: r.slug,
    title: r.title,
    streamUrl: r.stream_url,
    widthPx: r.width_px,
    heightPx: r.height_px,
    fps: r.fps,
    frameCount: r.frame_count,
    durationSeconds: Number(r.duration_seconds),
    format: r.format || 'ansi',
  };
}