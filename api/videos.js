import { createVideosHandler } from '../lib/videos.js';
import { listVideos } from '../lib/catalog.js';

export default createVideosHandler({
  listVideos,
  maxSeconds: Number(process.env.MAX_STREAM_SECONDS) || 285,
});