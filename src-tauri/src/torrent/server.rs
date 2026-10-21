//! A small HTTP server on 127.0.0.1 that hands torrent files to the player as they download.
//!
//! It answers `GET` and `HEAD` for `/stream/<token>` with support for `Range` requests, which is
//! all mpv needs to play and seek. Tokens are random, so other programs on the computer cannot
//! guess a URL. Every response closes the connection; mpv opens a new one per seek.

use std::future::Future;
use std::net::{Ipv4Addr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;

use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// A readable, seekable file and what the response needs to describe it.
pub struct Opened {
    pub len: u64,
    pub name: String,
    pub reader: Box<dyn ReadSeek>,
}

pub trait ReadSeek: AsyncRead + AsyncSeek + Unpin + Send {}
impl<T: AsyncRead + AsyncSeek + Unpin + Send> ReadSeek for T {}

pub type OpenFuture = Pin<Box<dyn Future<Output = Result<Opened, OpenError>> + Send>>;

/// Opens the file behind a token. Called once per request.
pub type Opener = Arc<dyn Fn(String) -> OpenFuture + Send + Sync>;

#[derive(Debug)]
pub enum OpenError {
    UnknownToken,
    Failed(String),
}

/// Largest request head accepted; mpv's are a few hundred bytes.
const MAX_HEAD: usize = 16 * 1024;

/// A response whose file sends nothing for this long is ended, so a torrent without peers
/// cannot hold its stream open forever.
const STALL_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Pause after a failed accept, so a lasting error does not spin the loop.
const ACCEPT_RETRY: Duration = Duration::from_millis(100);

/// Binds to a free port on 127.0.0.1 only and serves until the runtime stops.
pub async fn start(opener: Opener) -> std::io::Result<SocketAddr> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let addr = listener.local_addr()?;
    tokio::spawn(async move {
        loop {
            let Ok((socket, peer)) = listener.accept().await else {
                tokio::time::sleep(ACCEPT_RETRY).await;
                continue;
            };
            // Bound to loopback, so this cannot happen; refuse anything else regardless.
            if !peer.ip().is_loopback() {
                continue;
            }
            let opener = opener.clone();
            tokio::spawn(async move {
                let _ = serve(socket, opener).await;
            });
        }
    });
    Ok(addr)
}

/// A parsed `Range` header: the first and last byte wanted (inclusive).
#[derive(Debug, PartialEq)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,
}

/// Reads a single `bytes=` range against a file of `len` bytes. `Ok(None)` means no usable
/// range was asked for (serve everything); `Err` means the range cannot be satisfied.
pub fn parse_range(header: Option<&str>, len: u64) -> Result<Option<ByteRange>, ()> {
    let Some(spec) = header.and_then(|h| h.trim().strip_prefix("bytes=")) else {
        return Ok(None);
    };
    // Multiple ranges are not used by players; serve the whole file instead.
    if spec.contains(',') {
        return Ok(None);
    }
    let (from, to) = spec.split_once('-').ok_or(())?;
    let (from, to) = (from.trim(), to.trim());
    let range = if from.is_empty() {
        // "bytes=-500": the last 500 bytes.
        let suffix: u64 = to.parse().map_err(|_| ())?;
        if suffix == 0 {
            return Err(());
        }
        ByteRange {
            start: len.saturating_sub(suffix),
            end: len.saturating_sub(1),
        }
    } else {
        let start: u64 = from.parse().map_err(|_| ())?;
        let end = if to.is_empty() {
            len.saturating_sub(1)
        } else {
            to.parse::<u64>()
                .map_err(|_| ())?
                .min(len.saturating_sub(1))
        };
        ByteRange { start, end }
    };
    if len == 0 || range.start >= len || range.start > range.end {
        return Err(());
    }
    Ok(Some(range))
}

struct Request {
    head_only: bool,
    token: Option<String>,
    range: Option<String>,
}

fn parse_request(head: &str) -> Option<Request> {
    let mut lines = head.split("\r\n");
    let mut parts = lines.next()?.split_whitespace();
    let method = parts.next()?;
    let path = parts.next()?;
    if method != "GET" && method != "HEAD" {
        return None;
    }
    let token = path
        .strip_prefix("/stream/")
        .map(|t| t.split(['?', '/']).next().unwrap_or_default().to_string())
        .filter(|t| !t.is_empty());
    let range = lines
        .filter_map(|l| l.split_once(':'))
        .find(|(name, _)| name.trim().eq_ignore_ascii_case("range"))
        .map(|(_, value)| value.trim().to_string());
    Some(Request {
        head_only: method == "HEAD",
        token,
        range,
    })
}

fn content_type(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".mkv") {
        "video/x-matroska"
    } else if lower.ends_with(".mp4") || lower.ends_with(".m4v") {
        "video/mp4"
    } else if lower.ends_with(".webm") {
        "video/webm"
    } else if lower.ends_with(".avi") {
        "video/x-msvideo"
    } else {
        "application/octet-stream"
    }
}

async fn respond_status<W: AsyncWrite + Unpin>(
    socket: &mut W,
    status: &str,
    extra: &str,
) -> std::io::Result<()> {
    let reply =
        format!("HTTP/1.1 {status}\r\n{extra}Content-Length: 0\r\nConnection: close\r\n\r\n");
    socket.write_all(reply.as_bytes()).await
}

async fn read_head(socket: &mut TcpStream) -> std::io::Result<Option<String>> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        let n = socket.read(&mut chunk).await?;
        if n == 0 {
            return Ok(None);
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            return Ok(Some(String::from_utf8_lossy(&buf[..end]).into_owned()));
        }
        if buf.len() > MAX_HEAD {
            return Ok(None);
        }
    }
}

async fn serve(mut socket: TcpStream, opener: Opener) -> std::io::Result<()> {
    let Some(head) = read_head(&mut socket).await? else {
        return Ok(());
    };
    let (mut incoming, mut outgoing) = socket.into_split();
    // mpv sends nothing more after its request, so the read side ends only when it hangs up
    // (a seek or a closed player). The response, and with it the torrent stream and the
    // librqbit resources it holds, is then dropped instead of waiting for data nobody reads.
    let hung_up = async {
        let mut scratch = [0u8; 256];
        while let Ok(n) = incoming.read(&mut scratch).await {
            if n == 0 {
                break;
            }
        }
    };
    tokio::select! {
        result = respond(&head, opener, &mut outgoing) => result,
        () = hung_up => Ok(()),
    }
}

async fn respond<W: AsyncWrite + Unpin>(
    head: &str,
    opener: Opener,
    socket: &mut W,
) -> std::io::Result<()> {
    let socket = &mut *socket;
    let Some(request) = parse_request(head) else {
        return respond_status(socket, "400 Bad Request", "").await;
    };
    let Some(token) = request.token else {
        return respond_status(socket, "404 Not Found", "").await;
    };
    let mut opened = match opener(token).await {
        Ok(opened) => opened,
        Err(OpenError::UnknownToken) => return respond_status(socket, "404 Not Found", "").await,
        Err(OpenError::Failed(reason)) => {
            // The reason goes in the status line, where mpv's log shows it.
            let reason: String = reason
                .chars()
                .filter(|c| !c.is_control())
                .take(200)
                .collect();
            let status = format!("503 {reason}");
            return respond_status(socket, &status, "").await;
        }
    };
    let len = opened.len;
    let range = match parse_range(request.range.as_deref(), len) {
        Ok(range) => range,
        Err(()) => {
            let extra = format!("Content-Range: bytes */{len}\r\n");
            return respond_status(socket, "416 Range Not Satisfiable", &extra).await;
        }
    };
    let (status, start, count, content_range) = match range {
        Some(r) => (
            "206 Partial Content",
            r.start,
            r.end - r.start + 1,
            format!("Content-Range: bytes {}-{}/{len}\r\n", r.start, r.end),
        ),
        None => ("200 OK", 0, len, String::new()),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {}\r\nAccept-Ranges: bytes\r\n{content_range}Content-Length: {count}\r\nConnection: close\r\n\r\n",
        content_type(&opened.name)
    );
    socket.write_all(head.as_bytes()).await?;
    if request.head_only || count == 0 {
        return Ok(());
    }
    opened.reader.seek(std::io::SeekFrom::Start(start)).await?;
    let mut body = (&mut opened.reader).take(count);
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = tokio::time::timeout(STALL_TIMEOUT, body.read(&mut buf))
            .await
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "no data"))??;
        if n == 0 {
            break;
        }
        socket.write_all(&buf[..n]).await?;
    }
    socket.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn ranges() {
        let r = |h: &str| parse_range(Some(h), 1000);
        assert_eq!(parse_range(None, 1000), Ok(None));
        assert_eq!(r("bytes=0-"), Ok(Some(ByteRange { start: 0, end: 999 })));
        assert_eq!(
            r("bytes=100-199"),
            Ok(Some(ByteRange {
                start: 100,
                end: 199
            }))
        );
        // An end past the file is cut to the file.
        assert_eq!(
            r("bytes=900-5000"),
            Ok(Some(ByteRange {
                start: 900,
                end: 999
            }))
        );
        assert_eq!(
            r("bytes=-100"),
            Ok(Some(ByteRange {
                start: 900,
                end: 999
            }))
        );
        assert_eq!(r("bytes=1000-"), Err(()));
        assert_eq!(r("bytes=500-100"), Err(()));
        assert_eq!(r("bytes=abc-"), Err(()));
        assert_eq!(r("bytes=0-1,5-9"), Ok(None));
        assert_eq!(r("items=0-1"), Ok(None));
    }

    #[test]
    fn requests() {
        let req =
            parse_request("GET /stream/abc123 HTTP/1.1\r\nHost: x\r\nrange: bytes=5-\r\n").unwrap();
        assert_eq!(req.token.as_deref(), Some("abc123"));
        assert_eq!(req.range.as_deref(), Some("bytes=5-"));
        assert!(!req.head_only);
        assert!(
            parse_request("HEAD /stream/abc HTTP/1.1")
                .unwrap()
                .head_only
        );
        assert!(parse_request("POST /stream/abc HTTP/1.1").is_none());
        assert_eq!(parse_request("GET / HTTP/1.1").unwrap().token, None);
    }

    /// A file whose data never arrives, like a torrent piece nobody has. Flags when dropped.
    struct Stuck(Arc<std::sync::atomic::AtomicBool>);

    static STUCK_DROPPED: std::sync::LazyLock<Arc<std::sync::atomic::AtomicBool>> =
        std::sync::LazyLock::new(Default::default);

    impl Drop for Stuck {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    impl AsyncRead for Stuck {
        fn poll_read(
            self: Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            _: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Pending
        }
    }

    impl AsyncSeek for Stuck {
        fn start_seek(self: Pin<&mut Self>, _: std::io::SeekFrom) -> std::io::Result<()> {
            Ok(())
        }

        fn poll_complete(
            self: Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<u64>> {
            std::task::Poll::Ready(Ok(0))
        }
    }

    #[tokio::test]
    async fn a_player_hanging_up_frees_the_stream() {
        let addr = start(test_opener()).await.unwrap();
        let mut socket = TcpStream::connect(addr).await.unwrap();
        socket
            .write_all(b"GET /stream/stuck HTTP/1.1\r\n\r\n")
            .await
            .unwrap();
        let mut head = [0u8; 12];
        socket.read_exact(&mut head).await.unwrap();
        assert_eq!(&head, b"HTTP/1.1 200");
        assert!(!STUCK_DROPPED.load(std::sync::atomic::Ordering::SeqCst));
        drop(socket);
        for _ in 0..50 {
            if STUCK_DROPPED.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("the stream was still held after the player hung up");
    }

    fn test_opener() -> Opener {
        Arc::new(|token: String| -> OpenFuture {
            Box::pin(async move {
                match token.as_str() {
                    "good" => {
                        let data: Vec<u8> = (0..=255u8).cycle().take(10_000).collect();
                        Ok(Opened {
                            len: data.len() as u64,
                            name: "Movie.mkv".into(),
                            reader: Box::new(Cursor::new(data)),
                        })
                    }
                    "broken" => Err(OpenError::Failed("no peers".into())),
                    "stuck" => Ok(Opened {
                        len: 1_000_000,
                        name: "Movie.mkv".into(),
                        reader: Box::new(Stuck(STUCK_DROPPED.clone())),
                    }),
                    _ => Err(OpenError::UnknownToken),
                }
            })
        })
    }

    async fn fetch(addr: SocketAddr, request: &str) -> (String, Vec<u8>) {
        let mut socket = TcpStream::connect(addr).await.unwrap();
        socket.write_all(request.as_bytes()).await.unwrap();
        let mut response = Vec::new();
        socket.read_to_end(&mut response).await.unwrap();
        let split = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let head = String::from_utf8(response[..split].to_vec()).unwrap();
        (head, response[split + 4..].to_vec())
    }

    #[tokio::test]
    async fn serves_ranges_on_loopback() {
        let addr = start(test_opener()).await.unwrap();
        assert!(addr.ip().is_loopback());

        let (head, body) = fetch(addr, "GET /stream/good HTTP/1.1\r\n\r\n").await;
        assert!(head.starts_with("HTTP/1.1 200 OK"));
        assert!(head.contains("Content-Length: 10000"));
        assert!(head.contains("Content-Type: video/x-matroska"));
        assert_eq!(body.len(), 10_000);

        let (head, body) = fetch(
            addr,
            "GET /stream/good HTTP/1.1\r\nRange: bytes=300-309\r\n\r\n",
        )
        .await;
        assert!(head.starts_with("HTTP/1.1 206 Partial Content"));
        assert!(head.contains("Content-Range: bytes 300-309/10000"));
        assert_eq!(
            body,
            (300..310).map(|i| (i % 256) as u8).collect::<Vec<_>>()
        );

        let (head, body) = fetch(addr, "HEAD /stream/good HTTP/1.1\r\n\r\n").await;
        assert!(head.contains("Accept-Ranges: bytes"));
        assert!(body.is_empty());

        let (head, _) = fetch(
            addr,
            "GET /stream/good HTTP/1.1\r\nRange: bytes=20000-\r\n\r\n",
        )
        .await;
        assert!(head.starts_with("HTTP/1.1 416"));
        let (head, _) = fetch(addr, "GET /stream/nope HTTP/1.1\r\n\r\n").await;
        assert!(head.starts_with("HTTP/1.1 404"));
        let (head, _) = fetch(addr, "GET /stream/broken HTTP/1.1\r\n\r\n").await;
        assert!(head.starts_with("HTTP/1.1 503"));
    }
}
