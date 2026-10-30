//! Minimal HTTP/1.1 server for `/<name>`, `/stream` and `/healthz`.

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use framefeed_core::{DEFAULT_CELL, Fit, Pace, Pacer, Size, Viewport, plan_render};
use framefeed_decode::{Decoder, FfmpegDecoder, OutputSize, probe};
use framefeed_proto::{AnsiEncoder, KittyEncoder, KittyOptions, SixelEncoder};

use crate::media::Library;
use crate::params::{BackendChoice, StreamParams, parse_query, parse_stream_query};
use crate::source_guard::validate_source;

const MAX_HEAD_BYTES: usize = 16 * 1024;

/// Server policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerConfig {
    /// Allow `url` to be a local filesystem path.
    pub allow_local: bool,
    /// Maximum concurrent streams before answering 429.
    pub max_streams: usize,
    /// Bundled clips served at `/<name>`; `None` disables that route.
    pub media: Option<Library>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            allow_local: false,
            max_streams: 4,
            media: None,
        }
    }
}

/// Accepts connections forever, one thread per connection.
///
/// # Errors
///
/// Returns I/O errors raised by `accept` itself; per-connection errors are logged
/// and do not stop the server.
pub fn run(listener: TcpListener, config: ServerConfig) -> io::Result<()> {
    let active = Arc::new(AtomicUsize::new(0));
    for incoming in listener.incoming() {
        let stream = incoming?;
        let config = config.clone();
        let active = Arc::clone(&active);
        // One thread per connection: streams are independent decoders, and the
        // concurrency cap is enforced inside the handler.
        thread::spawn(move || {
            if let Err(error) = handle_connection(stream, &config, &active) {
                eprintln!("framefeed serve: connection error: {error}");
            }
        });
    }
    Ok(())
}

/// Serves one HTTP connection.
///
/// # Errors
///
/// Returns I/O errors from reading the request or writing the response headers.
pub fn handle_connection(
    mut stream: TcpStream,
    config: &ServerConfig,
    active: &AtomicUsize,
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    stream.set_nodelay(true)?;

    let head = read_head(&mut stream)?;
    let request_line = head.lines().next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or("/");
    if method != "GET" {
        return respond_text(&mut stream, 405, "Method Not Allowed", "only GET\n");
    }

    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    match path {
        "/healthz" => respond_text(&mut stream, 200, "OK", "ok\n"),
        "/" => respond_index(&mut stream, config),
        "/stream" => {
            let params = match parse_query(query) {
                Ok(params) => params,
                Err(error) => {
                    return respond_text(&mut stream, 400, "Bad Request", &format!("{error}\n"));
                }
            };
            if let Err(error) = validate_source(&params.source, config.allow_local) {
                return respond_text(&mut stream, 403, "Forbidden", &format!("{error}\n"));
            }
            stream_with_lease(&mut stream, config, active, params)
        }
        other => {
            let Some(file) = config
                .media
                .as_ref()
                .and_then(|library| library.resolve(other))
            else {
                return respond_not_found(&mut stream, config);
            };
            // A resolved library path is our own file, not user-supplied input, so it
            // skips the SSRF guard: the resolver already confined it to the directory.
            let params = match parse_stream_query(query, Some(file.display().to_string())) {
                Ok(params) => params,
                Err(error) => {
                    return respond_text(&mut stream, 400, "Bad Request", &format!("{error}\n"));
                }
            };
            stream_with_lease(&mut stream, config, active, params)
        }
    }
}

struct Lease<'a>(&'a AtomicUsize);

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn stream_with_lease(
    stream: &mut TcpStream,
    config: &ServerConfig,
    active: &AtomicUsize,
    params: StreamParams,
) -> io::Result<()> {
    if active.load(Ordering::SeqCst) >= config.max_streams {
        return respond_text(
            stream,
            429,
            "Too Many Requests",
            "too many concurrent streams\n",
        );
    }
    active.fetch_add(1, Ordering::SeqCst);
    let _lease = Lease(active);
    stream_video(stream, params)
}

fn stream_video(stream: &mut TcpStream, params: StreamParams) -> io::Result<()> {
    let Some(viewport) = Viewport::new(params.cols, params.rows, DEFAULT_CELL) else {
        return respond_text(stream, 400, "Bad Request", "invalid viewport\n");
    };
    let metadata = match probe(&params.source) {
        Ok(metadata) => metadata,
        Err(error) => return respond_text(stream, 502, "Bad Gateway", &format!("{error}\n")),
    };
    let plan = plan_render(
        viewport,
        Size::new(metadata.width, metadata.height),
        params.fit,
        Some(
            params
                .quality
                .target_height(viewport.pixel_size().height)
                .min(params.backend.budget().max_raster_height),
        ),
    );
    let mut decoder = match FfmpegDecoder::open(
        &params.source,
        Some(OutputSize {
            width: plan.raster.width,
            height: plan.raster.height,
            crop: params.fit == Fit::Cover,
        }),
    ) {
        Ok(decoder) => decoder,
        Err(error) => return respond_text(stream, 502, "Bad Gateway", &format!("{error}\n")),
    };

    let mut encoder = match params.backend {
        BackendChoice::Ansi => Encoder::Ansi(AnsiEncoder::new()),
        BackendChoice::Sixel => Encoder::Sixel(Box::new(SixelEncoder::new())),
        BackendChoice::Kitty => match KittyEncoder::new(KittyOptions::default()) {
            Ok(encoder) => Encoder::Kitty(encoder),
            Err(error) => {
                return respond_text(stream, 500, "Internal Server Error", &format!("{error}\n"));
            }
        },
    };

    let fps = metadata
        .fps
        .min(f64::from(params.fps.min(params.backend.budget().max_fps)))
        .max(1.0);
    let mut pacer = Pacer::new(fps, Instant::now());
    let cols = cells(plan.raster.width, viewport.cell.width_px, viewport.cols);
    let rows = cells(plan.raster.height, viewport.cell.height_px, viewport.rows);

    stream.write_all(
        b"HTTP/1.1 200 OK\r\n\
          Content-Type: text/plain; charset=utf-8\r\n\
          Cache-Control: no-store\r\n\
          X-Content-Type-Options: nosniff\r\n\
          Connection: close\r\n\r\n",
    )?;
    stream.write_all(b"\x1b[?25l\x1b[2J\x1b[H")?;
    stream.flush()?;

    let mut buffer = Vec::new();
    loop {
        let frame = match decoder.next_frame() {
            Ok(Some(frame)) => frame,
            Ok(None) => break,
            Err(error) => {
                eprintln!("framefeed serve: decode error: {error}");
                break;
            }
        };
        if let Pace::Wait(delay) = pacer.tick(Instant::now()) {
            thread::sleep(delay);
        }
        buffer.clear();
        match &mut encoder {
            Encoder::Ansi(encoder) => encoder.encode_frame(frame, &mut buffer),
            Encoder::Sixel(encoder) => encoder.encode_frame(frame, &mut buffer),
            Encoder::Kitty(encoder) => {
                if let Err(error) = encoder.encode_frame(frame, cols, rows, &mut buffer) {
                    eprintln!("framefeed serve: encode error: {error}");
                    break;
                }
            }
        }
        // A disconnected client is the normal end of a stream.
        if stream.write_all(&buffer).is_err() {
            break;
        }
    }

    let _ = stream.write_all(b"\x1b[0m\x1b[?25h");
    let _ = stream.flush();
    Ok(())
}

enum Encoder {
    Ansi(AnsiEncoder),
    Kitty(KittyEncoder),
    Sixel(Box<SixelEncoder>),
}

fn cells(pixels: u32, cell_px: u16, fallback: u16) -> u16 {
    let cell_px = u32::from(cell_px);
    if cell_px == 0 {
        return fallback;
    }
    u16::try_from(pixels.div_ceil(cell_px)).unwrap_or(fallback)
}

const PARAMS_HELP: &str = "parameters: q=144p|240p|360p|480p|720p|1080p|auto, \
backend=ansi|sixel|kitty, fps=1-60, cols=<20-300>, rows=<5-100>, fit=contain|cover|stretch\n";

fn respond_index(stream: &mut TcpStream, config: &ServerConfig) -> io::Result<()> {
    let mut body = String::from("framefeed - video in your terminal\n\n");
    body.push_str("  GET /<name>        stream a bundled clip (no argument needed)\n");
    body.push_str("  GET /stream?url=   relay any http(s) source\n");
    body.push_str("  GET /healthz       liveness probe\n\n");
    match &config.media {
        Some(library) => {
            let names = library.names();
            if names.is_empty() {
                body.push_str("clips: none found in the library directory\n");
            } else {
                body.push_str("clips:\n");
                for name in names {
                    body.push_str(&format!("  /{name}\n"));
                }
            }
        }
        None => body.push_str("clips: library disabled (start serve with --media <dir>)\n"),
    }
    body.push('\n');
    body.push_str(PARAMS_HELP);
    body.push_str("\nexample: curl -N http://127.0.0.1:8080/big-buck-bunny?q=480p&backend=sixel\n");
    respond_text(stream, 200, "OK", &body)
}

fn respond_not_found(stream: &mut TcpStream, config: &ServerConfig) -> io::Result<()> {
    let mut body = String::from("no such clip\n\n");
    if let Some(library) = &config.media {
        let names = library.names();
        if !names.is_empty() {
            body.push_str("available: ");
            body.push_str(&names.join(", "));
            body.push('\n');
        }
    }
    body.push_str("listing and parameters: /  |  arbitrary sources: /stream?url=\n");
    respond_text(stream, 404, "Not Found", &body)
}

fn read_head(stream: &mut TcpStream) -> io::Result<String> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if buffer.len() > MAX_HEAD_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request head too large",
            ));
        }
    }
    Ok(String::from_utf8_lossy(&buffer).into_owned())
}

fn respond_text(stream: &mut TcpStream, status: u16, reason: &str, body: &str) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    )?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::{ServerConfig, handle_connection};
    use crate::media::Library;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::path::PathBuf;
    use std::sync::atomic::AtomicUsize;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn with_config(config: ServerConfig, path: &str) -> (String, String) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handle_connection(stream, &config, &AtomicUsize::new(0)).unwrap();
        });
        let mut client = TcpStream::connect(address).unwrap();
        write!(client, "GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        server.join().unwrap();
        let (head, body) = response.split_once("\r\n\r\n").unwrap_or((&response, ""));
        (head.to_string(), body.to_string())
    }

    fn request(path: &str) -> (String, String) {
        with_config(ServerConfig::default(), path)
    }

    fn library_dir(files: &[&str]) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("framefeed-http-{unique}"));
        std::fs::create_dir_all(&root).unwrap();
        for name in files {
            std::fs::write(root.join(name), b"not really video").unwrap();
        }
        root
    }

    #[test]
    fn health_endpoint_answers_200() {
        let (head, body) = request("/healthz");
        assert!(head.starts_with("HTTP/1.1 200 OK"), "{head}");
        assert_eq!(body, "ok\n");
    }

    #[test]
    fn index_lists_clips_and_parameters() {
        let root = library_dir(&["clip.mp4"]);
        let (head, body) = with_config(
            ServerConfig {
                media: Some(Library::new(&root)),
                ..ServerConfig::default()
            },
            "/",
        );
        assert!(head.starts_with("HTTP/1.1 200 OK"), "{head}");
        assert!(body.contains("/clip"), "{body}");
        assert!(body.contains("backend=sixel"), "{body}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn media_route_rejects_unknown_and_traversal_names() {
        let root = library_dir(&["clip.mp4"]);
        let config = || ServerConfig {
            media: Some(Library::new(&root)),
            ..ServerConfig::default()
        };
        let (head, body) = with_config(config(), "/nope");
        assert!(head.starts_with("HTTP/1.1 404"), "{head}");
        assert!(body.contains("available: clip"), "{body}");
        assert!(
            with_config(config(), "/../clip.mp4")
                .0
                .starts_with("HTTP/1.1 404")
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn unknown_path_is_404_and_missing_url_is_400() {
        assert!(request("/nope").0.starts_with("HTTP/1.1 404"));
        let (head, body) = request("/stream?url=./clip.mp4");
        assert!(head.starts_with("HTTP/1.1 403"), "{head}");
        assert!(body.contains("--allow-local"), "{body}");
        assert!(request("/stream?q=360p").0.starts_with("HTTP/1.1 400"));
        assert!(
            request("/stream?url=http%3A%2F%2F127.0.0.1%2Fx&q=360p")
                .0
                .starts_with("HTTP/1.1 403")
        );
    }

    #[test]
    fn response_is_not_chunked_and_closes_the_connection() {
        let (head, _) = request("/healthz");
        assert!(head.contains("Connection: close"));
        assert!(!head.contains("Transfer-Encoding"));
    }
}
