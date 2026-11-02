import './env.mjs';
import { spawn, spawnSync } from 'node:child_process';
import { pipeline } from 'node:stream/promises';
import { createWriteStream, mkdirSync, statSync, existsSync, readFileSync } from 'node:fs';
import { createGzip } from 'node:zlib';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { neon } from '@neondatabase/serverless';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');

function parseArgs(argv) {
  const args = {
    file: null,
    slug: null,
    title: null,
    description: '',
    cols: 80,
    rows: 24,
    fps: 24,
    start: 0,
    duration: null,
    dryRun: false,
    ffmpeg: null,
    ffprobe: null,
  };
  const positional = [];
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--file') args.file = argv[++i];
    else if (a === '--slug') args.slug = argv[++i];
    else if (a === '--title') args.title = argv[++i];
    else if (a === '--description') args.description = argv[++i];
    else if (a === '--cols') args.cols = Number(argv[++i]);
    else if (a === '--rows') args.rows = Number(argv[++i]);
    else if (a === '--fps') args.fps = Number(argv[++i]);
    else if (a === '--start') args.start = Number(argv[++i]);
    else if (a === '--duration') args.duration = Number(argv[++i]);
    else if (a === '--dry-run') args.dryRun = true;
    else if (a === '--ffmpeg') args.ffmpeg = argv[++i];
    else if (a === '--ffprobe') args.ffprobe = argv[++i];
    else if (a.startsWith('--')) {
      console.error('Unknown option:', a);
      process.exit(1);
    } else {
      positional.push(a);
    }
  }
  if (!args.file && positional.length) args.file = positional[0];
  if (!args.title && positional.length > 1) args.title = positional.slice(1).join(' ');
  return args;
}

function slugify(s) {
  return String(s || '')
    .normalize('NFKD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .replace(/-+/g, '-')
    .slice(0, 63)
    .replace(/-+$/g, '');
}

function cleanTitle(s) {
  return String(s || '')
    .replace(/\.[^.]+$/, '')
    .replace(/[-_]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

function fail(msg) {
  console.error(msg);
  process.exit(1);
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (!args.file) fail('Missing --file <path>');
  if (!existsSync(args.file)) fail('Input file not found: ' + args.file);

  const cols = args.cols;
  const rows = args.rows;
  const fps = args.fps;
  if (!Number.isInteger(cols) || cols <= 0) fail('cols must be positive integer');
  if (!Number.isInteger(rows) || rows <= 0) fail('rows must be positive integer');
  if (!Number.isFinite(fps) || fps < 5 || fps > 30) fail('fps must be between 5 and 30');
  if (!Number.isFinite(args.start) || args.start < 0) fail('start must be >= 0');
  if (args.duration != null && (!Number.isFinite(args.duration) || args.duration <= 0))
    fail('duration must be > 0');

  const ffmpegPath = args.ffmpeg || process.env.FRAMEFEED_FFMPEG || 'ffmpeg';
  const ffprobePath = args.ffprobe || process.env.FRAMEFEED_FFPROBE || 'ffprobe';

  const baseName = path.basename(args.file);
  if (!args.title) args.title = cleanTitle(baseName);
  if (!args.slug) args.slug = slugify(args.title || baseName);
  if (!/^[a-z0-9][a-z0-9-]{0,62}$/.test(args.slug)) fail('Invalid slug: ' + args.slug);

  const width = cols;
  const height = rows * 2;

  // ffprobe
  const probeRes = spawnSync(
    ffprobePath,
    ['-v', 'error', '-show_entries', 'format=duration,size', '-of', 'json', args.file],
    { encoding: 'utf8' }
  );
  if (probeRes.error) {
    console.error('ffprobe could not be executed:', probeRes.error.message);
    console.error('Executable:', ffprobePath);
    console.error('Set --ffprobe or FRAMEFEED_FFPROBE to the full path.');
    process.exit(1);
  }
  if (probeRes.status !== 0) {
    console.error('ffprobe failed:', (probeRes.stderr || '').slice(0, 500));
    console.error('Executable:', ffprobePath);
    process.exit(1);
  }
  let probe = {};
  try {
    probe = JSON.parse(probeRes.stdout || '{}');
  } catch {}
  const srcDuration = probe?.format?.duration ? Number(probe.format.duration) : null;
  const srcSize = probe?.format?.size ? Number(probe.format.size) : null;
  console.log('Source:', args.file);
  if (srcDuration != null) console.log('  duration:', srcDuration.toFixed(2) + 's');
  if (srcSize != null) console.log('  size:', srcSize + ' bytes');

  // extract
  const outDir = path.join(ROOT, 'target', 'vercel');
  mkdirSync(outDir, { recursive: true });
  const gzPath = path.join(outDir, args.slug + '.bin.gz');
  const jsonPath = path.join(outDir, args.slug + '.json');

  const ffArgs = [
    '-hide_banner',
    '-loglevel',
    'error',
    '-nostats',
    '-ss',
    String(args.start),
    '-i',
    args.file,
  ];
  if (args.duration != null) {
    ffArgs.push('-t', String(args.duration));
  }
  ffArgs.push(
    '-an',
    '-sn',
    '-dn',
    '-vf',
    `fps=${fps},scale=${width}:${height}:force_original_aspect_ratio=decrease,pad=${width}:${height}:(ow-iw)/2:(oh-ih)/2:color=black,setsar=1`,
    '-pix_fmt',
    'rgb565le',
    '-f',
    'rawvideo',
    'pipe:1'
  );

  console.log('Extracting frames...');
  let rawBytes = 0;
  const child = spawn(ffmpegPath, ffArgs, { stdio: ['ignore', 'pipe', 'inherit'] });
  child.on('error', (err) => {
    console.error('ffmpeg could not be executed:', err.message);
    console.error('Executable:', ffmpegPath);
    console.error('Set --ffmpeg or FRAMEFEED_FFMPEG to the full path.');
  });
  const gz = createGzip();
  const out = createWriteStream(gzPath);
  child.stdout.on('data', (c) => {
    rawBytes += c.length;
  });

  let pipelineError = null;
  let exitCode = 0;
  await Promise.all([
    pipeline(child.stdout, gz, out).catch((err) => {
      pipelineError = err;
    }),
    new Promise((resolve) => {
      child.on('close', (code) => {
        exitCode = code == null ? 0 : code;
        resolve();
      });
    }),
  ]);
  if (pipelineError) {
    fail('pipeline failed: ' + (pipelineError.message || String(pipelineError)));
  }
  if (exitCode !== 0) {
    fail('ffmpeg failed with exit code ' + exitCode);
  }

  const frameBytes = width * height * 2;
  if (rawBytes % frameBytes !== 0) {
    fail(`Raw byte count ${rawBytes} is not a multiple of frame size ${frameBytes}`);
  }
  const frameCount = rawBytes / frameBytes;
  const durationSeconds = frameCount / fps;
  const sizeBytes = statSync(gzPath).size;

  const metadata = {
    slug: args.slug,
    title: args.title,
    description: args.description || '',
    cols,
    rows,
    fps,
    widthPx: width,
    heightPx: height,
    frameCount,
    durationSeconds,
    sizeBytes,
    createdAt: new Date().toISOString(),
  };

  // write json
  const { writeFileSync } = await import('node:fs');
  writeFileSync(jsonPath, JSON.stringify(metadata, null, 2) + '\n');

  console.log('Frames:', frameCount);
  console.log('Duration:', durationSeconds.toFixed(3) + 's');
  console.log('Gzip size:', sizeBytes + ' bytes');
  console.log('Wrote:', gzPath);
  console.log('Wrote:', jsonPath);

  if (durationSeconds > 285) {
    console.warn(
      'Warning: duration exceeds 285s. On Vercel Hobby functions stop at 300s; use Pro with MAX_STREAM_SECONDS=780 and maxDuration "max" for this clip.'
    );
  }

  if (args.dryRun) {
    console.log('Dry run: skipping upload and database.');
    console.log('Local preview: node scripts/dev-server.mjs');
    process.exit(0);
  }

  const token = process.env.BLOB_READ_WRITE_TOKEN;
  const dbUrl = process.env.DATABASE_URL || process.env.POSTGRES_URL || process.env.NEON_DATABASE_URL;
  if (!token) fail('BLOB_READ_WRITE_TOKEN is required');
  if (!dbUrl) fail('DATABASE_URL is required');

  // read old stream_url
  const sql = neon(dbUrl);
  let oldUrl = null;
  try {
    const rows = await sql('SELECT stream_url FROM videos WHERE slug = $1', [args.slug]);
    if (rows && rows.length) oldUrl = rows[0].stream_url;
  } catch (err) {
    fail('DB read failed: ' + (err && err.message ? err.message : String(err)));
  }

  // upload
  const { put, del } = await import('@vercel/blob');
  console.log('Uploading to Blob...');
  const gzBuffer = readFileSync(gzPath);
  const blob = await put(`videos/${args.slug}/frames.bin.gz`, gzBuffer, {
    access: 'public',
    token,
    addRandomSuffix: true,
    contentType: 'application/gzip',
    cacheControlMaxAge: 31536000,
  });
  const streamUrl = blob.url;
  console.log('Uploaded:', streamUrl);

  // upsert
  await sql(
    `INSERT INTO videos (slug,title,description,stream_url,width_px,height_px,cols,rows,fps,frame_count,duration_seconds,size_bytes,published,created_at,updated_at)
     VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,true,now(),now())
     ON CONFLICT (slug) DO UPDATE SET
       title=EXCLUDED.title,
       description=EXCLUDED.description,
       stream_url=EXCLUDED.stream_url,
       width_px=EXCLUDED.width_px,
       height_px=EXCLUDED.height_px,
       cols=EXCLUDED.cols,
       rows=EXCLUDED.rows,
       fps=EXCLUDED.fps,
       frame_count=EXCLUDED.frame_count,
       duration_seconds=EXCLUDED.duration_seconds,
       size_bytes=EXCLUDED.size_bytes,
       published=true,
       updated_at=now()`,
    [
      args.slug,
      args.title,
      args.description || '',
      streamUrl,
      width,
      height,
      cols,
      rows,
      fps,
      frameCount,
      durationSeconds,
      sizeBytes,
    ]
  );

  if (oldUrl && oldUrl !== streamUrl) {
    try {
      await del(oldUrl, { token });
      console.log('Deleted old blob.');
    } catch (err) {
      console.warn('Failed to delete old blob:', err && err.message ? err.message : String(err));
    }
  }

  console.log('Published:', args.slug);
  console.log('Play: curl -N https://<your-domain>/' + args.slug);
}

main().catch((err) => {
  console.error('publish failed:', err && err.message ? err.message : String(err));
  process.exit(1);
});