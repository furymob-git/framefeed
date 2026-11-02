export function createVideosHandler({ listVideos, maxSeconds = 285 } = {}) {
  return async function handler(req, res) {
    if (req.method !== 'GET') {
      res.statusCode = 405;
      res.setHeader('Allow', 'GET');
      res.end();
      return;
    }
    try {
      const videos = await listVideos();
      const body = JSON.stringify({ videos, maxSeconds });
      res.statusCode = 200;
      res.setHeader('Content-Type', 'application/json');
      res.setHeader('Cache-Control', 'no-store');
      res.end(body);
    } catch (err) {
      console.error('videos handler error:', err && err.message ? err.message : String(err));
      res.statusCode = 503;
      res.setHeader('Content-Type', 'application/json');
      res.end(JSON.stringify({ error: 'catalog unavailable' }));
    }
  };
}