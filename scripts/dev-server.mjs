import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import zlib from 'node:zlib';
import { createStreamHandler } from '../lib/stream.js';
import { createVideosHandler } from '../lib/videos.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const PUBLIC_DIR = path.join(ROOT, 'public');
const FIXTURES_DIR = path.join(ROOT, 'target', 'vercel');

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'application/javascript; charset=utf-8',
  '.mjs': 'application/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon',
  '.txt': 'text/plain; charset=utf-8',
  '.gz': 'application/gzip',
};

function loadFixtures() {
  const fixtures = new Map();
  if (!fs.existsSync(FIXTURES_DIR)) return fixtures;
  for (const f of fs.readdirSync(FIXTURES_DIR)) {
    if (!f.endsWith('.json')) continue;
    const slug = f.replace(/\.json$/, '');
    const jsonPath = path.join(FIXTURES_DIR, f);
    const gzPath = path.join(FIXTURES_DIR, slug + '.bin.gz');
    if (!fs.existsSync(gzPath)) continue;
    try {
      const meta = JSON.parse(fs.readFileSync(jsonPath, 'utf8'));
      fixtures.set(slug, { meta, gzPath });
    } catch {}
  }
  return fixtures;
}

export function startDevServer({ port = 0 } = {}) {
  const fixtures = loadFixtures();
  const maxSeconds = Number(process.env.MAX_STREAM_SECONDS) || 285;

  let serverPort = port;

  const listVideos = async () =>
    [...fixtures.values()].map(({ meta }) => ({
      slug: meta.slug,
      title: meta.title,
      description: meta.description || '',
      cols: meta.cols,
      rows: meta.rows,
      fps: meta.fps,
      durationSeconds: Number(meta.durationSeconds),
      sizeBytes: Number(meta.sizeBytes),
      createdAt: meta.createdAt,
    }));

  const getVideo = async (slug) => {
    const fx = fixtures.get(slug);
    if (!fx) return null;
    const m = fx.meta;
    return {
      slug: m.slug,
      title: m.title,
      streamUrl: `http://127.0.0.1:${serverPort}/__frames/${m.slug}.bin.gz`,
      widthPx: m.widthPx,
      heightPx: m.heightPx,
      fps: m.fps,
      frameCount: m.frameCount,
      durationSeconds: Number(m.durationSeconds),
    };
  };

  const streamHandler = createStreamHandler({ getVideo, maxSeconds, fetchImpl: globalThis.fetch });
  const videosHandler = createVideosHandler({ listVideos, maxSeconds });

  const server = http.createServer(async (req, res) => {
    try {
      const url = new URL(req.url || '/', 'http://localhost');
      const pathname = decodeURIComponent(url.pathname);

      if (pathname === '/healthz') {
        res.statusCode = 200;
        res.setHeader('Content-Type', 'text/plain; charset=utf-8');
        res.end('ok\n');
        return;
      }

      if (pathname === '/api/videos') {
        await videosHandler(req, res);
        return;
      }

      if (pathname.startsWith('/__frames/')) {
        const name = pathname.slice('/__frames/'.length);
        if (name.includes('/') || name.includes('..')) {
          res.statusCode = 400;
          res.end('bad path');
          return;
        }
        const fx = fixtures.get(name.replace(/\.bin\.gz$/, ''));
        const gzPath = fx ? fx.gzPath : path.join(FIXTURES_DIR, name);
        if (!fs.existsSync(gzPath)) {
          res.statusCode = 404;
          res.end('not found');
          return;
        }
        const stat = fs.statSync(gzPath);
        res.statusCode = 200;
        res.setHeader('Content-Type', 'application/gzip');
        res.setHeader('Content-Length', String(stat.size));
        fs.createReadStream(gzPath).pipe(res);
        return;
      }

      // slug route: if not static file and not api, treat as slug
      // Serve static public files
      if (pathname.includes('..') || pathname.includes('\\')) {
        res.statusCode = 400;
        res.end('bad path');
        return;
      }
      const rel = pathname.replace(/^\//, '');
      const resolved = path.resolve(PUBLIC_DIR, rel === '' ? 'index.html' : rel);
      if (resolved === path.resolve(PUBLIC_DIR) || !resolved.startsWith(path.resolve(PUBLIC_DIR) + path.sep)) {
        // fall through
      } else if (fs.existsSync(resolved) && fs.statSync(resolved).isFile()) {
        const ext = path.extname(resolved).toLowerCase();
        res.statusCode = 200;
        res.setHeader('Content-Type', MIME[ext] || 'application/octet-stream');
        fs.createReadStream(resolved).pipe(res);
        return;
      }

      if (pathname === '/') {
        const index = path.join(PUBLIC_DIR, 'index.html');
        if (fs.existsSync(index)) {
          res.statusCode = 200;
          res.setHeader('Content-Type', 'text/html; charset=utf-8');
          fs.createReadStream(index).pipe(res);
        } else {
          res.statusCode = 404;
          res.end('not found');
        }
        return;
      }

      // treat as slug
      const slug = pathname.replace(/^\//, '');
      if (fixtures.has(slug)) {
        await streamHandler(req, res);
        return;
      }

      // maybe a slug not in fixtures but format valid -> streamHandler returns 404
      if (/^[a-z0-9][a-z0-9-]{0,62}$/.test(slug)) {
        await streamHandler(req, res);
        return;
      }

      res.statusCode = 404;
      res.end('not found');
    } catch (err) {
      console.error('dev server error:', err && err.message ? err.message : String(err));
      if (!res.headersSent) {
        res.statusCode = 500;
        res.end('error');
      } else {
        try {
          res.end();
        } catch {}
      }
    }
  });

  return new Promise((resolve) => {
    server.listen(port, '127.0.0.1', () => {
      serverPort = server.address().port;
      const url = `http://127.0.0.1:${serverPort}`;
      if (port === 0 || !port) {
        // only print if not test? print anyway
      }
      console.log(`Dev server listening on ${url}`);
      if (fixtures.size === 0) {
        console.log('No fixtures found in target/vercel. Run publish --dry-run to create some.');
      } else {
        console.log('Fixtures:', [...fixtures.keys()].join(', '));
        for (const s of fixtures.keys()) console.log(`  ${url}/${s}`);
      }
      resolve({
        url,
        close: () =>
          new Promise((res2) => {
            server.close(() => res2());
          }),
      });
    });
  });
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  const port = Number(process.env.PORT) || 3000;
  startDevServer({ port }).then(({ url }) => {
    console.log('Ready');
  });
}