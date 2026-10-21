//! The built-in torrent engine (librqbit) and the local server that streams its files to mpv.
//!
//! Nothing starts until the first torrent is played. The engine accepts no incoming peer
//! connections, uses no UPnP and no local peer discovery; DHT is on unless switched off in
//! Settings (its UDP socket is the only one that listens on the network). The stream server
//! binds to 127.0.0.1 only.

pub mod server;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, DhtSessionConfig, Session, SessionOptions,
};
use rand::Rng;
use serde::Deserialize;
use tokio::sync::OnceCell;

use server::{OpenError, OpenFuture, Opened, Opener};

/// How long to wait for a magnet's file list from peers before giving up.
const METADATA_TIMEOUT: Duration = Duration::from_secs(60);

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
    let mut seen = HashSet::new();
    for tracker in source
        .sources
        .iter()
        .filter_map(|s| s.strip_prefix("tracker:"))
    {
        let tracker = tracker.trim();
        let allowed = ["udp://", "http://", "https://"]
            .iter()
            .any(|scheme| tracker.starts_with(scheme));
        if allowed && seen.insert(tracker.to_string()) {
            uri.push_str("&tr=");
            uri.push_str(&percent_encode(tracker));
        }
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

fn random_token() -> String {
    let bytes: [u8; 16] = rand::rng().random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub struct TorrentEngine {
    cache_dir: PathBuf,
    session: OnceCell<Arc<Session>>,
    server: OnceCell<String>,
    streams: Mutex<HashMap<String, TorrentSource>>,
    /// Read when the session starts; changing it takes effect on the next app start.
    dht: Mutex<bool>,
    /// Peers to connect to directly, so tests can use a local seeder without DHT or trackers.
    #[cfg(test)]
    initial_peers: Mutex<Vec<std::net::SocketAddr>>,
}

impl TorrentEngine {
    pub fn new(cache_dir: PathBuf, dht: bool) -> Arc<Self> {
        Arc::new(Self {
            cache_dir,
            session: OnceCell::new(),
            server: OnceCell::new(),
            streams: Mutex::new(HashMap::new()),
            dht: Mutex::new(dht),
            #[cfg(test)]
            initial_peers: Mutex::new(Vec::new()),
        })
    }

    pub fn set_dht(&self, enabled: bool) {
        *self.dht.lock().unwrap() = enabled;
    }

    async fn session(&self) -> Result<Arc<Session>, String> {
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
    async fn open(&self, token: &str) -> Result<Opened, OpenError> {
        let source = self
            .streams
            .lock()
            .unwrap()
            .get(token)
            .cloned()
            .ok_or(OpenError::UnknownToken)?;
        let failed = |e: String| OpenError::Failed(e);
        let session = self.session().await.map_err(failed)?;
        let magnet = magnet_uri(&source).map_err(failed)?;
        #[allow(unused_mut)]
        let mut options = AddTorrentOptions {
            only_files: source.file_idx.map(|i| vec![i]),
            overwrite: true,
            ..AddTorrentOptions::default()
        };
        #[cfg(test)]
        {
            options.initial_peers = Some(self.initial_peers.lock().unwrap().clone());
        }
        let handle = match session
            .add_torrent(AddTorrent::from_url(magnet), Some(options))
            .await
            .map_err(|e| failed(format!("could not add the torrent: {e}")))?
        {
            AddTorrentResponse::Added(_, handle)
            | AddTorrentResponse::AlreadyManaged(_, handle) => handle,
            AddTorrentResponse::ListOnly(_) => {
                return Err(failed("the torrent was only listed".to_string()))
            }
        };
        tokio::time::timeout(METADATA_TIMEOUT, handle.wait_until_initialized())
            .await
            .map_err(|_| failed("no peers sent the torrent's file list in time".to_string()))?
            .map_err(|e| failed(format!("the torrent could not start: {e}")))?;

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
        if source.file_idx != Some(file) {
            // Only download the file being watched.
            let only: HashSet<usize> = [file].into();
            session
                .update_only_files(&handle, &only)
                .await
                .map_err(|e| failed(e.to_string()))?;
        }
        let stream = handle
            .clone()
            .stream(file)
            .await
            .map_err(|e| failed(format!("could not stream the file: {e}")))?;
        Ok(Opened {
            len: stream.len(),
            name: files[file].0.clone(),
            reader: Box::new(stream),
        })
    }
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

        let engine = TorrentEngine::new(root.join("cache"), false);
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
