//! The built-in torrent engine (librqbit) and the local server that streams its files to mpv.
//!
//! Nothing starts until the first torrent is played. The engine accepts no incoming peer
//! connections, uses no UPnP and no local peer discovery; DHT is on unless switched off in
//! Settings (its UDP socket is the only one that listens on the network). The stream server
//! binds to 127.0.0.1 only.

mod cache;
pub mod server;

use std::collections::{HashMap, HashSet};
use std::io::SeekFrom;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, DhtSessionConfig, ManagedTorrent, Session,
    SessionOptions,
};
use rand::Rng;
use serde::Deserialize;
use tokio::io::{AsyncRead, AsyncSeek, ReadBuf};
use tokio::sync::OnceCell;

use server::{OpenError, OpenFuture, Opened, Opener};

/// How long to wait for a magnet's file list from peers before giving up.
const METADATA_TIMEOUT: Duration = Duration::from_secs(60);

/// How often the cache size is checked while the engine runs.
const CACHE_CHECK_INTERVAL: Duration = Duration::from_secs(30);

const GIB: u64 = 1024 * 1024 * 1024;

const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "m4v", "avi", "webm", "mov", "ts", "m2ts", "wmv",
];

/// A torrent stream as addons describe it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentSource {
    pub info_hash: String,
    /// Which file of the torrent to play. When missing, the largest video file is used.
    pub file_idx: Option<usize>,
    /// The addon's `sources`: `tracker:<url>` and `dht:<hash>` entries.
    #[serde(default)]
    pub sources: Vec<String>,
}

/// Builds a magnet link with the trackers the addon listed.
pub fn magnet_uri(source: &TorrentSource) -> Result<String, String> {
    let hash = source.info_hash.trim().to_ascii_lowercase();
    if hash.len() != 40 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("the torrent has an invalid info hash".to_string());
    }
    let mut uri = format!("magnet:?xt=urn:btih:{hash}");
    for tracker in trackers(source) {
        uri.push_str("&tr=");
        uri.push_str(&percent_encode(&tracker));
    }
    Ok(uri)
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// The file to play: the requested one if it exists, else the largest video file, else the
/// largest file.
pub fn choose_file(files: &[(String, u64)], requested: Option<usize>) -> Option<usize> {
    if let Some(index) = requested.filter(|i| *i < files.len()) {
        return Some(index);
    }
    let is_video = |name: &str| {
        name.rsplit('.')
            .next()
            .is_some_and(|ext| VIDEO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
    };
    let largest = |videos_only: bool| {
        files
            .iter()
            .enumerate()
            .filter(|(_, (name, _))| !videos_only || is_video(name))
            .max_by_key(|(_, (_, len))| *len)
            .map(|(i, _)| i)
    };
    largest(true).or_else(|| largest(false))
}

/// Whether one part of a torrent's file name can only ever name a file inside the torrent's
/// folder. librqbit rejects ".." and slashes, but on Windows "C:name" (a drive-relative path)
/// would leave the cache folder, and device names such as "CON" do not name files at all.
pub fn safe_name_part(part: &str) -> bool {
    const DEVICES: &[&str] = &["con", "prn", "aux", "nul", "conin$", "conout$"];
    let stem = part
        .split('.')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let device = DEVICES.contains(&stem.as_str())
        || ((stem.starts_with("com") || stem.starts_with("lpt"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit());
    !part.is_empty()
        && part != "."
        && part != ".."
        && !device
        && !part.chars().any(|c| {
            c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
        })
}

/// Marks a torrent as in use while it opens, so the cache cleanup leaves it alone.
struct InUse(Arc<AtomicUsize>);

impl InUse {
    fn new(counter: Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self(counter)
    }
}

impl Drop for InUse {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn random_token() -> String {
    let bytes: [u8; 16] = rand::rng().random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// What the cache cleanup needs to know about one torrent.
struct Usage {
    handle: Arc<ManagedTorrent>,
    last_used: Instant,
    open_streams: Arc<AtomicUsize>,
}

pub struct TorrentEngine {
    cache_dir: PathBuf,
    session: OnceCell<Arc<Session>>,
    server: OnceCell<String>,
    streams: Mutex<HashMap<String, TorrentSource>>,
    /// Torrents added this run, by info hash.
    usage: Mutex<HashMap<String, Usage>>,
    /// Read when the session starts; changing it takes effect on the next app start.
    dht: Mutex<bool>,
    /// Most bytes the downloaded torrents may take before idle ones are deleted.
    cache_limit: AtomicU64,
    /// Peers to connect to directly, so tests can use a local seeder without DHT or trackers.
    #[cfg(test)]
    initial_peers: Mutex<Vec<std::net::SocketAddr>>,
}

/// A torrent file stream that counts itself as open while mpv reads from it.
struct CountedStream<S> {
    inner: S,
    open_streams: Arc<AtomicUsize>,
}

impl<S> CountedStream<S> {
    fn new(inner: S, open_streams: Arc<AtomicUsize>) -> Self {
        open_streams.fetch_add(1, Ordering::SeqCst);
        Self {
            inner,
            open_streams,
        }
    }
}

impl<S> Drop for CountedStream<S> {
    fn drop(&mut self) {
        self.open_streams.fetch_sub(1, Ordering::SeqCst);
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for CountedStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<S: AsyncSeek + Unpin> AsyncSeek for CountedStream<S> {
    fn start_seek(mut self: Pin<&mut Self>, position: SeekFrom) -> std::io::Result<()> {
        Pin::new(&mut self.inner).start_seek(position)
    }

    fn poll_complete(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
        Pin::new(&mut self.inner).poll_complete(cx)
    }
}

impl TorrentEngine {
    /// Creates the engine. Anything left in `cache_dir` from an earlier run (for example after
    /// a crash) is deleted: nothing is resumed between runs.
    pub fn new(cache_dir: PathBuf, dht: bool, cache_limit_gb: u32) -> Arc<Self> {
        let _ = std::fs::remove_dir_all(&cache_dir);
        Arc::new(Self {
            cache_dir,
            session: OnceCell::new(),
            server: OnceCell::new(),
            streams: Mutex::new(HashMap::new()),
            usage: Mutex::new(HashMap::new()),
            dht: Mutex::new(dht),
            cache_limit: AtomicU64::new(u64::from(cache_limit_gb.max(1)) * GIB),
            #[cfg(test)]
            initial_peers: Mutex::new(Vec::new()),
        })
    }

    pub fn set_dht(&self, enabled: bool) {
        *self.dht.lock().unwrap() = enabled;
    }

    pub fn set_cache_limit(&self, gb: u32) {
        self.cache_limit
            .store(u64::from(gb.max(1)) * GIB, Ordering::SeqCst);
    }

    /// Deletes idle torrents, least recently watched first, while the cache is over its limit.
    pub async fn enforce_cache_limit(&self) {
        let Some(session) = self.session.get() else {
            return;
        };
        let now = Instant::now();
        let victims = {
            let usage = self.usage.lock().unwrap();
            let entries: Vec<cache::CacheEntry<String>> = usage
                .iter()
                .map(|(hash, u)| cache::CacheEntry {
                    key: hash.clone(),
                    size: u.handle.stats().progress_bytes,
                    last_used: u.last_used,
                    open_streams: u.open_streams.load(Ordering::SeqCst),
                })
                .collect();
            cache::evictions(&entries, self.cache_limit.load(Ordering::SeqCst), now)
        };
        for hash in victims {
            // A torrent may have started opening again since the list was made.
            let removed = {
                let mut usage = self.usage.lock().unwrap();
                let idle = usage
                    .get(&hash)
                    .is_some_and(|u| u.open_streams.load(Ordering::SeqCst) == 0);
                if idle {
                    usage.remove(&hash)
                } else {
                    None
                }
            };
            if let Some(usage) = removed {
                let _ = session.delete(usage.handle.id().into(), true).await;
            }
        }
    }

    /// The player closed: stop downloading torrents nobody is watching, then trim the cache.
    pub async fn pause_idle(&self) {
        let Some(session) = self.session.get() else {
            return;
        };
        let idle: Vec<Arc<ManagedTorrent>> = self
            .usage
            .lock()
            .unwrap()
            .values()
            .filter(|u| u.open_streams.load(Ordering::SeqCst) == 0)
            .map(|u| u.handle.clone())
            .collect();
        for handle in idle {
            if !handle.is_paused() {
                let _ = session.pause(&handle).await;
            }
        }
        self.enforce_cache_limit().await;
    }

    /// Stops the engine and deletes everything it downloaded. Called when the app exits.
    pub async fn shutdown(&self) {
        if let Some(session) = self.session.get() {
            session.stop().await;
        }
        let _ = std::fs::remove_dir_all(&self.cache_dir);
    }

    async fn session(self: &Arc<Self>) -> Result<Arc<Session>, String> {
        self.session
            .get_or_try_init(|| async {
                let dht = *self.dht.lock().unwrap();
                let options = |ipv4_only: bool| SessionOptions {
                    dht: dht.then(|| DhtSessionConfig {
                        persistence: None,
                        ..DhtSessionConfig::default()
                    }),
                    // No incoming peer connections, so nothing listens for peers.
                    listen: None,
                    disable_local_service_discovery: true,
                    persistence: None,
                    fastresume: false,
                    ipv4_only,
                    ..SessionOptions::default()
                };
                // Dual-stack sockets fail where IPv6 is turned off; fall back to IPv4 only.
                let session =
                    match Session::new_with_opts(self.cache_dir.clone(), options(false)).await {
                        Ok(session) => session,
                        Err(_) => Session::new_with_opts(self.cache_dir.clone(), options(true))
                            .await
                            .map_err(|e| format!("could not start the torrent engine: {e}"))?,
                    };
                // Check the cache size regularly for as long as the engine runs.
                let weak = Arc::downgrade(self);
                tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(CACHE_CHECK_INTERVAL).await;
                        let Some(engine) = weak.upgrade() else {
                            return;
                        };
                        engine.enforce_cache_limit().await;
                    }
                });
                Ok::<_, String>(session)
            })
            .await
            .cloned()
    }

    /// Returns the local URL mpv should open for this torrent. Nothing is downloaded until
    /// the player requests the URL.
    pub async fn register(self: &Arc<Self>, source: TorrentSource) -> Result<String, String> {
        magnet_uri(&source)?;
        let base = self
            .server
            .get_or_try_init(|| async {
                let engine = Arc::downgrade(self);
                let opener: Opener = Arc::new(move |token: String| -> OpenFuture {
                    let engine = engine.clone();
                    Box::pin(async move {
                        match engine.upgrade() {
                            Some(engine) => engine.open(&token).await,
                            None => Err(OpenError::UnknownToken),
                        }
                    })
                });
                server::start(opener)
                    .await
                    .map(|addr| format!("http://{addr}"))
                    .map_err(|e| format!("could not start the local stream server: {e}"))
            })
            .await?;
        let token = random_token();
        self.streams.lock().unwrap().insert(token.clone(), source);
        Ok(format!("{base}/stream/{token}"))
    }

    /// Adds the torrent (once), waits for its file list and opens the chosen file as a stream.
    async fn open(self: &Arc<Self>, token: &str) -> Result<Opened, OpenError> {
        let source = self
            .streams
            .lock()
            .unwrap()
            .get(token)
            .cloned()
            .ok_or(OpenError::UnknownToken)?;
        let failed = |e: String| OpenError::Failed(e);
        let hash = source.info_hash.trim().to_ascii_lowercase();
        // A torrent watched before is in use from now on, so the cache cleanup cannot delete
        // it while it opens.
        let _in_use = self.usage.lock().unwrap().get_mut(&hash).map(|u| {
            u.last_used = Instant::now();
            InUse::new(u.open_streams.clone())
        });
        let session = self.session().await.map_err(failed)?;
        let magnet = magnet_uri(&source).map_err(failed)?;
        // Fetching the file list from peers can take forever for a dead torrent.
        let (handle, managed_before) =
            tokio::time::timeout(METADATA_TIMEOUT, self.add(&session, magnet, &source))
                .await
                .map_err(|_| {
                    failed("no peers sent the torrent's file list in time".to_string())
                })??;

        let files: Vec<(String, u64)> = handle
            .with_metadata(|m| {
                m.file_infos
                    .iter()
                    .map(|f| (f.relative_filename.to_string_lossy().into_owned(), f.len))
                    .collect()
            })
            .map_err(|e| failed(e.to_string()))?;
        let file = choose_file(&files, source.file_idx)
            .ok_or_else(|| failed("the torrent has no files".to_string()))?;
        if managed_before || source.file_idx != Some(file) {
            // Only download the file being watched (another episode of a season pack may have
            // been selected before).
            let only: HashSet<usize> = [file].into();
            session
                .update_only_files(&handle, &only)
                .await
                .map_err(|e| failed(e.to_string()))?;
        }
        // A torrent paused when the player closed resumes when it is watched again.
        if handle.is_paused() {
            session
                .unpause(&handle)
                .await
                .map_err(|e| failed(e.to_string()))?;
        }
        let open_streams = {
            let mut usage = self.usage.lock().unwrap();
            let entry = usage
                .entry(handle.info_hash().as_string())
                .or_insert_with(|| Usage {
                    handle: handle.clone(),
                    last_used: Instant::now(),
                    open_streams: Arc::new(AtomicUsize::new(0)),
                });
            entry.last_used = Instant::now();
            entry.open_streams.clone()
        };
        let stream = handle
            .clone()
            .stream(file)
            .await
            .map_err(|e| failed(format!("could not stream the file: {e}")))?;
        Ok(Opened {
            len: stream.len(),
            name: files[file].0.clone(),
            reader: Box::new(CountedStream::new(stream, open_streams)),
        })
    }

    /// Adds the torrent unless it is already in the session, and waits until its file list is
    /// known. New torrents are first only listed, so their file names can be checked before
    /// anything is written. Returns whether the torrent was already in the session.
    async fn add(
        &self,
        session: &Arc<Session>,
        magnet: String,
        source: &TorrentSource,
    ) -> Result<(Arc<ManagedTorrent>, bool), OpenError> {
        let failed = |e: String| OpenError::Failed(e);
        let hash = source.info_hash.trim().to_ascii_lowercase();
        let base = || {
            #[allow(unused_mut)]
            let mut options = AddTorrentOptions {
                only_files: source.file_idx.map(|i| vec![i]),
                overwrite: true,
                // The info hash names the folder; the torrent's own name could leave the cache.
                sub_folder: Some(hash.clone()),
                ..AddTorrentOptions::default()
            };
            #[cfg(test)]
            {
                options.initial_peers = Some(self.initial_peers.lock().unwrap().clone());
            }
            options
        };
        let listed = session
            .add_torrent(
                AddTorrent::from_url(magnet),
                Some(AddTorrentOptions {
                    list_only: true,
                    ..base()
                }),
            )
            .await
            .map_err(|e| failed(format!("could not add the torrent: {e}")))?;
        let (handle, managed_before) = match listed {
            AddTorrentResponse::AlreadyManaged(_, handle) => (handle, true),
            AddTorrentResponse::Added(_, handle) => (handle, false),
            AddTorrentResponse::ListOnly(list) => {
                let unsafe_name = list.info.iter_file_details().any(|f| {
                    f.filename
                        .iter_components()
                        .any(|part| !safe_name_part(&part))
                });
                if unsafe_name {
                    return Err(failed(
                        "the torrent has file names that could leave the download folder"
                            .to_string(),
                    ));
                }
                let mut options = base();
                // Peers that answered already, so the file list is not looked up again.
                let mut peers = options.initial_peers.take().unwrap_or_default();
                peers.extend(list.seen_peers);
                options.initial_peers = Some(peers);
                options.trackers = Some(trackers(source));
                match session
                    .add_torrent(AddTorrent::from_bytes(list.torrent_bytes), Some(options))
                    .await
                    .map_err(|e| failed(format!("could not add the torrent: {e}")))?
                {
                    AddTorrentResponse::Added(_, handle) => (handle, false),
                    AddTorrentResponse::AlreadyManaged(_, handle) => (handle, true),
                    AddTorrentResponse::ListOnly(_) => {
                        return Err(failed("the torrent was only listed".to_string()))
                    }
                }
            }
        };
        handle
            .wait_until_initialized()
            .await
            .map_err(|e| failed(format!("the torrent could not start: {e}")))?;
        Ok((handle, managed_before))
    }
}

/// The addon's trackers that use a tracker protocol, each once.
fn trackers(source: &TorrentSource) -> Vec<String> {
    let mut seen = HashSet::new();
    source
        .sources
        .iter()
        .filter_map(|s| s.strip_prefix("tracker:"))
        .map(str::trim)
        .filter(|t| {
            ["udp://", "http://", "https://"]
                .iter()
                .any(|s| t.starts_with(s))
        })
        .filter(|t| seen.insert(t.to_string()))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "ABCDEF0123456789ABCDEF0123456789ABCDEF01";

    fn source(sources: &[&str]) -> TorrentSource {
        TorrentSource {
            info_hash: HASH.into(),
            file_idx: None,
            sources: sources.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn magnet_with_trackers() {
        let uri = magnet_uri(&source(&[
            "tracker:udp://tracker.example:1337/announce",
            "tracker:udp://tracker.example:1337/announce",
            "tracker:https://t.example/announce?x=1",
            "tracker:file:///etc/passwd",
            "dht:abcdef0123456789abcdef0123456789abcdef01",
        ]))
        .unwrap();
        assert_eq!(
            uri,
            "magnet:?xt=urn:btih:abcdef0123456789abcdef0123456789abcdef01\
             &tr=udp%3A%2F%2Ftracker.example%3A1337%2Fannounce\
             &tr=https%3A%2F%2Ft.example%2Fannounce%3Fx%3D1"
        );
    }

    #[test]
    fn file_names_must_stay_in_the_folder() {
        for ok in [
            "Show.S01E01.mkv",
            "Season 1",
            "a.b.c",
            "Concert.mkv",
            "com10.txt",
        ] {
            assert!(safe_name_part(ok), "{ok}");
        }
        for bad in [
            "",
            ".",
            "..",
            "C:evil.dll",
            "a:b",
            "x\\y",
            "x/y",
            "CON",
            "nul.txt",
            "COM1",
            "lpt9.mkv",
            "a\u{0}b",
            "what?",
        ] {
            assert!(!safe_name_part(bad), "{bad}");
        }
    }

    #[test]
    fn rejects_bad_hashes() {
        let mut bad = source(&[]);
        bad.info_hash = "not-a-hash".into();
        assert!(magnet_uri(&bad).is_err());
    }

    #[test]
    fn chooses_files() {
        let files = vec![
            ("Show/Sample.mkv".to_string(), 50_000_000),
            ("Show/Show.S01E01.mkv".to_string(), 1_500_000_000),
            ("Show/Extras.zip".to_string(), 3_000_000_000),
            ("Show/info.nfo".to_string(), 2_000),
        ];
        assert_eq!(choose_file(&files, Some(0)), Some(0));
        // The largest video, not the larger archive.
        assert_eq!(choose_file(&files, None), Some(1));
        assert_eq!(choose_file(&files, Some(99)), Some(1));
        let no_video = vec![("a.zip".to_string(), 5), ("b.rar".to_string(), 9)];
        assert_eq!(choose_file(&no_video, None), Some(1));
        assert_eq!(choose_file(&[], None), None);
    }

    /// Seeds a generated file from a second local session and plays it through the engine and
    /// the local stream server, including a seek (range request) into the middle.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn streams_a_torrent_from_a_local_seeder() {
        use librqbit::spawn_utils::BlockingSpawner;
        use librqbit::{create_torrent, CreateTorrentOptions, ListenerMode, ListenerOptions};
        use std::net::Ipv4Addr;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        // librqbit opens its UDP tracker socket on IPv6, which some sandboxes cannot create.
        if std::net::UdpSocket::bind("[::]:0").is_err() {
            eprintln!("skipped: this machine cannot create IPv6 sockets");
            return;
        }

        let root = std::env::temp_dir().join(format!("spaceport-torrent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let seed_dir = root.join("seed");
        std::fs::create_dir_all(seed_dir.join("Movie")).unwrap();
        let data: Vec<u8> = (0..3_000_000u32).map(|i| (i * 7 % 251) as u8).collect();
        std::fs::write(seed_dir.join("Movie/Movie.2024.1080p.mkv"), &data).unwrap();
        std::fs::write(seed_dir.join("Movie/readme.txt"), b"hello").unwrap();

        let created = create_torrent(
            &seed_dir.join("Movie"),
            CreateTorrentOptions {
                name: Some("Movie"),
                trackers: vec![],
                piece_length: Some(256 * 1024),
            },
            &BlockingSpawner::new(2),
        )
        .await
        .unwrap();
        let info_hash = created.info_hash().as_string();

        let seeder = Session::new_with_opts(
            seed_dir.clone(),
            SessionOptions {
                dht: None,
                listen: Some(ListenerOptions {
                    mode: ListenerMode::TcpOnly,
                    listen_addr: (Ipv4Addr::LOCALHOST, 0).into(),
                    ipv4_only: true,
                    ..ListenerOptions::default()
                }),
                disable_local_service_discovery: true,
                ipv4_only: true,
                ..SessionOptions::default()
            },
        )
        .await
        .unwrap();
        let seeded = seeder
            .add_torrent(
                AddTorrent::from_bytes(created.as_bytes().unwrap()),
                Some(AddTorrentOptions {
                    output_folder: Some(created.output_folder.to_string_lossy().into_owned()),
                    overwrite: true,
                    ..AddTorrentOptions::default()
                }),
            )
            .await
            .unwrap()
            .into_handle()
            .unwrap();
        seeded.wait_until_initialized().await.unwrap();
        let seeder_addr = seeder.listen_addr().expect("seeder listens");

        let engine = TorrentEngine::new(root.join("cache"), false, 10);
        engine.initial_peers.lock().unwrap().push(seeder_addr);
        let url = engine
            .register(TorrentSource {
                info_hash,
                file_idx: None,
                sources: vec![],
            })
            .await
            .unwrap();
        assert!(url.starts_with("http://127.0.0.1:"));

        let address = url
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap()
            .to_string();
        let path = &url[url.find("/stream/").unwrap()..];
        let request = format!("GET {path} HTTP/1.1\r\nRange: bytes=1500000-1500099\r\n\r\n");
        let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
        socket.write_all(request.as_bytes()).await.unwrap();
        let mut response = Vec::new();
        tokio::time::timeout(Duration::from_secs(60), socket.read_to_end(&mut response))
            .await
            .expect("stream answered in time")
            .unwrap();
        let split = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let head = String::from_utf8_lossy(&response[..split]).into_owned();
        assert!(head.starts_with("HTTP/1.1 206"), "{head}");
        assert!(
            head.contains("Content-Range: bytes 1500000-1500099/3000000"),
            "{head}"
        );
        assert_eq!(&response[split + 4..], &data[1_500_000..1_500_100]);

        // Closing the app deletes everything the engine downloaded.
        assert!(root.join("cache").exists());
        engine.shutdown().await;
        assert!(!root.join("cache").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn tokens_are_random_hex() {
        let (a, b) = (random_token(), random_token());
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }
}
