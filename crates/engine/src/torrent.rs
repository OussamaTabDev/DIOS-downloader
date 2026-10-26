//! BitTorrent: `.torrent` files and magnet links, downloaded by librqbit
//! (DHT, trackers, uTP and UPnP port mapping are all built in).
//!
//! One shared session starts lazily with the first torrent. Every torrent
//! download of the list is one torrent inside that session; the manager
//! starts / pauses it with the queue and maps its stats to the usual
//! progress, speed and peer display.

use crate::model::TorrentSource;
use anyhow::{anyhow, bail, Context};
use librqbit::limits::LimitsConfig;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, DhtSessionConfig, ListenerMode,
    ListenerOptions, Session, SessionOptions, TorrentStatsState,
};
/// librqbit calls this ManagedTorrentHandle (an alias that is not itself
/// re-exported).
type ManagedTorrentHandle = Arc<librqbit::ManagedTorrent>;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How long a magnet link may take to find its torrent info (DHT / trackers).
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(90);

/// One file of a torrent, for the file list windows.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TorrentFileInfo {
    pub index: usize,
    /// path inside the torrent ("movie.mkv", "subs/en.srt")
    pub path: String,
    pub size: u64,
}

/// What a torrent contains, read from its metadata.
#[derive(Debug, Clone)]
pub struct TorrentMeta {
    pub name: String,
    /// info hash in hex; finds the torrent again after a restart
    pub info_hash: String,
    /// all files together
    pub total_size: u64,
    pub files: Vec<TorrentFileInfo>,
}

impl TorrentMeta {
    /// The bytes of the chosen files (all of them when `files` is empty).
    pub fn size_of(&self, files: &[usize]) -> u64 {
        if files.is_empty() {
            return self.total_size;
        }
        self.files.iter().filter(|f| files.contains(&f.index)).map(|f| f.size).sum()
    }
}

/// Speed limits for the torrents, KB/s (0 = unlimited). From Options.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TorrentLimits {
    pub download_kb: u64,
    pub upload_kb: u64,
}

impl TorrentLimits {
    fn bps(kb: u64) -> Option<NonZeroU32> {
        let bps = kb.saturating_mul(1024).min(u32::MAX as u64) as u32;
        NonZeroU32::new(bps)
    }
    fn ratelimits(&self) -> LimitsConfig {
        LimitsConfig {
            download_bps: Self::bps(self.download_kb),
            upload_bps: Self::bps(self.upload_kb),
        }
    }
}

/// The BitTorrent session owner. Cheap to create; the real session
/// (with DHT and listeners) starts with the first torrent.
pub struct TorrentEngine {
    data_dir: PathBuf,
    session: Mutex<Option<Arc<Session>>>,
}

impl TorrentEngine {
    pub fn new(data_dir: PathBuf) -> Arc<Self> {
        Arc::new(Self { data_dir, session: Mutex::new(None) })
    }

    /// Where the app keeps the .torrent files it was given, so a download
    /// survives the original file being moved or deleted.
    pub fn library_dir(&self) -> PathBuf {
        let dir = self.data_dir.join("torrents");
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    /// The shared session: DHT, a TCP + uTP listener with UPnP port
    /// mapping, trackers. Created on first use with the current settings.
    pub async fn session(&self, limits: TorrentLimits) -> anyhow::Result<Arc<Session>> {
        if let Some(s) = self.session.lock().unwrap().clone() {
            return Ok(s);
        }
        let mut listen = ListenerOptions::default();
        // uTP as well: many peers only answer over it
        listen.mode = ListenerMode::TcpAndUtp;
        listen.enable_upnp_port_forwarding = true;
        let opts = SessionOptions {
            dht: Some(DhtSessionConfig { bootstrap_addrs: None, port: None, persistence: None }),
            listen: Some(listen),
            ratelimits: limits.ratelimits(),
            ..Default::default()
        };
        let folder = self.data_dir.join("torrents");
        let session = Session::new_with_opts(folder, opts)
            .await
            .context("could not start the BitTorrent session")?;
        *self.session.lock().unwrap() = Some(session.clone());
        Ok(session)
    }

    /// The running session, if there is one (to pause / remove torrents).
    pub fn session_if_running(&self) -> Option<Arc<Session>> {
        self.session.lock().unwrap().clone()
    }

    /// Add the torrent (or find the one already in the session) in a paused
    /// state, with the chosen folder and files. A magnet link waits for its
    /// torrent info here; that can take up to a minute and a half.
    pub async fn add_paused(
        &self,
        source: &TorrentSource,
        work_dir: &Path,
        files: Vec<usize>,
        limits: TorrentLimits,
    ) -> anyhow::Result<ManagedTorrentHandle> {
        let timeout = match source {
            TorrentSource::Magnet(_) => RESOLVE_TIMEOUT,
            TorrentSource::Torrent(_) => Duration::from_secs(30),
        };
        match tokio::time::timeout(
            timeout,
            self.add_paused_inner(source, work_dir, files, limits),
        )
        .await
        {
            Ok(r) => r,
            Err(_) => bail!(
                "took too long to get the torrent info{} - check the link and try again",
                if matches!(source, TorrentSource::Magnet(_)) {
                    " (no peers answered)"
                } else {
                    ""
                }
            ),
        }
    }

    async fn add_paused_inner(
        &self,
        source: &TorrentSource,
        work_dir: &Path,
        files: Vec<usize>,
        limits: TorrentLimits,
    ) -> anyhow::Result<ManagedTorrentHandle> {
        let session = self.session(limits).await?;
        let opts = AddTorrentOptions {
            paused: true,
            // resume writing into the partly downloaded files
            overwrite: true,
            only_files: (!files.is_empty()).then_some(files),
            output_folder: Some(work_dir.display().to_string()),
            ratelimits: limits.ratelimits(),
            ..Default::default()
        };
        let add = match source {
            TorrentSource::Torrent(p) => AddTorrent::from_local_filename(&p.display().to_string())
                .with_context(|| format!("could not read {}", p.display()))?,
            TorrentSource::Magnet(m) => AddTorrent::from_url(m.clone()),
        };
        match session.add_torrent(add, Some(opts)).await? {
            AddTorrentResponse::Added(_, h) | AddTorrentResponse::AlreadyManaged(_, h) => Ok(h),
            AddTorrentResponse::ListOnly(_) => bail!("unexpected answer for a torrent"),
        }
    }

    /// Make the session forget the torrent (its files stay unless
    /// `delete_files` is true).
    pub async fn remove(&self, info_hash_hex: &str, delete_files: bool) {
        let Some(session) = self.session_if_running() else { return };
        let Ok(hash) = parse_info_hash(info_hash_hex) else { return };
        let _ = session.delete(librqbit::api::TorrentIdOrHash::Hash(hash), delete_files).await;
    }

    /// Change which files of a torrent are downloaded (empty = all of them).
    pub async fn update_files(&self, info_hash_hex: &str, files: Vec<usize>) {
        let Some(session) = self.session_if_running() else { return };
        let Ok(hash) = parse_info_hash(info_hash_hex) else { return };
        let Some(handle) = session.get(librqbit::api::TorrentIdOrHash::Hash(hash)) else { return };
        let set: std::collections::HashSet<usize> = files.into_iter().collect();
        let _ = session.update_only_files(&handle, &set).await;
    }
}

/// The info hash from its hex text.
pub fn parse_info_hash(hex: &str) -> anyhow::Result<librqbit::dht::Id20> {
    let bytes = hex_bytes(hex).ok_or_else(|| anyhow!("broken torrent info hash"))?;
    Ok(librqbit::dht::Id20::new(bytes))
}

fn hex_bytes(s: &str) -> Option<[u8; 20]> {
    if s.len() != 40 {
        return None;
    }
    let mut out = [0u8; 20];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

/// Read the torrent's contents from its handle (metadata must have arrived).
pub fn meta_of(handle: &ManagedTorrentHandle) -> anyhow::Result<TorrentMeta> {
    let hash = handle.info_hash().as_string();
    handle.with_metadata(|m| TorrentMeta {
        name: m.info.name().map(|n| n.into_owned()).unwrap_or_else(|| "torrent".into()),
        info_hash: hash.clone(),
        total_size: m.info.lengths().total_length(),
        files: m
            .file_infos
            .iter()
            .enumerate()
            .map(|(i, f)| TorrentFileInfo {
                index: i,
                path: f.relative_filename.display().to_string(),
                size: f.len,
            })
            .collect(),
    })
}

/// Is this torrent still comparing what is on the disk with what it needs?
pub fn is_checking(stats: &librqbit::TorrentStats) -> bool {
    matches!(stats.state, TorrentStatsState::Initializing { .. })
}

/// The torrent's error text, when it has one.
pub fn error_of(stats: &librqbit::TorrentStats) -> Option<String> {
    if let TorrentStatsState::Error = stats.state {
        return Some(stats.error.clone().unwrap_or_else(|| "the torrent failed".into()));
    }
    None
}

/// Bytes present of the chosen files (all of them when `selection` is
/// empty). While the torrent is still checking, the whole-torrent progress
/// is scaled instead: the per-file numbers are not known yet.
pub fn selected_have(stats: &librqbit::TorrentStats, selection: &[usize], wanted: u64) -> u64 {
    if stats.file_progress.is_empty() {
        return if stats.total_bytes > 0 {
            (stats.progress_bytes as f64 * wanted as f64 / stats.total_bytes as f64) as u64
        } else {
            0
        };
    }
    if selection.is_empty() {
        return stats.progress_bytes;
    }
    selection.iter().filter_map(|&i| stats.file_progress.get(i).copied()).sum()
}

/// Per-file have bytes, aligned with the file list (0 while checking).
pub fn file_progress(stats: &librqbit::TorrentStats) -> Vec<u64> {
    stats.file_progress.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_hash_round_trip() {
        let hex = "0123456789abcdef0123456789abcdef01234567";
        let id = parse_info_hash(hex).unwrap();
        assert_eq!(id.as_string(), hex);
        assert!(parse_info_hash("zz").is_err());
    }

    #[test]
    fn limits_convert() {
        let l = TorrentLimits { download_kb: 500, upload_kb: 0 };
        let r = l.ratelimits();
        assert_eq!(r.download_bps.map(|b| b.get()), Some(512_000));
        assert_eq!(r.upload_bps, None);
    }
}
