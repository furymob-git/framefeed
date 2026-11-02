import { createStreamHandler } from '../lib/stream.js';
import { getVideo } from '../lib/catalog.js';

export default createStreamHandler({
  getVideo,
  maxSeconds: Number(process.env.MAX_STREAM_SECONDS) || 285,
});