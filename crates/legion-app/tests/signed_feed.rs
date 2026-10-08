//! Signed HTTP update feed against a local server.
//!
//! The filename deliberately does not contain "update": Windows
//! installer detection auto-elevates executables whose names contain that
//! substring.
//!
//! Signing keys are generated in-process and are never written to the repo.
//! The production feed URL and verifying key stay owner configuration.

#![cfg(feature = "updater-http")]

use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ed25519_dalek::Signer;
use legion_app::AppProductMode;
use legion_app::updater::{
    FetchInstallOutcome, HttpInstallRequest, HttpManifestSource, MANIFEST_FILE, ManifestSource,
    UpdateError, UpdateFeedConfig, UpdatePolicy, UpdatePoll, Updater,
};
use legion_protocol::{ReleaseArtifact, ReleaseManifestV1};

fn thread_token() -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::thread::current().id().hash(&mut hasher);
    hasher.finish()
}

fn temp_root(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "legion_feed_{}_{}_{}_{tag}",
        std::process::id(),
        nanos,
        thread_token()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(data))
}

fn runtime_signing_key() -> ed25519_dalek::SigningKey {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(std::process::id().to_le_bytes());
    hasher.update(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_le_bytes(),
    );
    hasher.update(thread_token().to_le_bytes());
    let stack = &hasher as *const _ as usize;
    hasher.update(stack.to_le_bytes());
    let digest = hasher.finalize();
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&digest);
    ed25519_dalek::SigningKey::from_bytes(&seed)
}

fn launcher(exit_code: i32, marker: &str) -> (String, Vec<u8>) {
    if cfg!(windows) {
        let bytes = format!("@echo off\r\nREM {marker}\r\nexit /b {exit_code}\r\n").into_bytes();
        ("legion-pkg.cmd".to_string(), bytes)
    } else {
        let bytes = format!("#!/bin/sh\n# {marker}\nexit {exit_code}\n").into_bytes();
        ("legion-pkg".to_string(), bytes)
    }
}

fn signed_manifest(
    key: &ed25519_dalek::SigningKey,
    version: &str,
    artifact_name: &str,
    artifact_path: &str,
    hashed_bytes: &[u8],
) -> (Vec<u8>, Vec<u8>) {
    let artifact = ReleaseArtifact::new(
        artifact_name.to_string(),
        "test".to_string(),
        "test-target".to_string(),
        artifact_path.to_string(),
        sha256_hex(hashed_bytes),
    );
    let manifest = ReleaseManifestV1::new(
        "legion-desktop".to_string(),
        "stable".to_string(),
        version.to_string(),
        None,
        Some("0.1.0".to_string()),
        vec![artifact],
        "2026-10-08T00:00:00Z".to_string(),
        Some("OWNER-PROVIDED".to_string()),
    );
    let bytes = toml::to_string_pretty(&manifest).unwrap().into_bytes();
    let signature = key.sign(&bytes).to_bytes().to_vec();
    (bytes, signature)
}

struct LocalFeed {
    manifest_url: String,
    addr: std::net::SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl LocalFeed {
    fn serve(routes: HashMap<String, Vec<u8>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            while !stop_flag.load(Ordering::SeqCst) {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                if stop_flag.load(Ordering::SeqCst) {
                    break;
                }
                let request = read_headers(&mut stream);
                let path = request.split_whitespace().nth(1).unwrap_or("");
                if let Some(body) = routes.get(path) {
                    respond(&mut stream, "200 OK", body);
                } else {
                    respond(&mut stream, "404 Not Found", b"");
                }
            }
        });
        Self {
            manifest_url: format!("http://{addr}/{MANIFEST_FILE}"),
            addr,
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for LocalFeed {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn read_headers(stream: &mut TcpStream) -> String {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buf = Vec::new();
    let mut scratch = [0u8; 1024];
    loop {
        match stream.read(&mut scratch) {
            Ok(0) => break,
            Ok(read) => {
                buf.extend_from_slice(&scratch[..read]);
                if buf.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            Err(_) => break,
        }
        if buf.len() > 64 * 1024 {
            break;
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
}

fn respond(stream: &mut TcpStream, status: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

struct Slot {
    root: PathBuf,
    current: PathBuf,
    previous: PathBuf,
    journal: PathBuf,
    work: PathBuf,
}

impl Slot {
    fn new(original: &[u8], file_name: &str) -> Self {
        let root = temp_root("slot");
        let current = root.join(file_name);
        fs::write(&current, original).unwrap();
        Self {
            previous: root.join(format!("{file_name}.previous")),
            journal: root.join("journal.toml"),
            work: root.join("work"),
            root,
            current,
        }
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn policy(version: &str) -> UpdatePolicy {
    UpdatePolicy {
        current_version: version.to_string(),
        current_channel: "stable".to_string(),
        allow_unsigned_beta: false,
    }
}

fn install(
    feed: &LocalFeed,
    key: &ed25519_dalek::SigningKey,
    slot: &Slot,
    version: &str,
    artifact_name: &str,
    mode: AppProductMode,
) -> Result<FetchInstallOutcome, UpdateError> {
    let source =
        HttpManifestSource::with_timeout(&feed.manifest_url, Duration::from_secs(5)).unwrap();
    Updater::new().fetch_stage_swap_and_launch(HttpInstallRequest {
        mode,
        source: &source,
        policy: &policy(version),
        verifying_key: &key.verifying_key().to_bytes(),
        work_dir: &slot.work,
        artifact_name,
        current_path: &slot.current,
        previous_path: &slot.previous,
        journal_path: &slot.journal,
        now_utc: "2026-10-08T00:00:00Z",
    })
}

fn journal_versions(path: &Path) -> (String, Option<String>) {
    let text = fs::read_to_string(path).unwrap();
    let value: toml::Value = toml::from_str(&text).unwrap();
    let journal = &value["journal"];
    let current = journal["current_version"].as_str().unwrap().to_string();
    let previous = journal["previous_version"].as_str().map(str::to_string);
    (current, previous)
}

fn routes(
    manifest: Vec<u8>,
    signature: Vec<u8>,
    artifact_path: &str,
    artifact: Vec<u8>,
) -> HashMap<String, Vec<u8>> {
    let mut map = HashMap::new();
    map.insert(format!("/{MANIFEST_FILE}"), manifest);
    map.insert(format!("/{MANIFEST_FILE}.sig"), signature);
    map.insert(format!("/{artifact_path}"), artifact);
    map
}

#[test]
fn signed_feed_valid_update_swaps_launches_and_keeps_previous() {
    let key = runtime_signing_key();
    let (file_name, original) = launcher(0, "N");
    let (_name, incoming) = launcher(0, "N-plus");
    let (manifest, signature) = signed_manifest(&key, "0.2.0", "pkg", &file_name, &incoming);
    let feed = LocalFeed::serve(routes(manifest, signature, &file_name, incoming.clone()));
    let slot = Slot::new(&original, &file_name);

    let outcome = install(&feed, &key, &slot, "0.1.0", "pkg", AppProductMode::Assist).unwrap();
    let FetchInstallOutcome::Installed(installed) = outcome else {
        panic!("expected an installed package, got {outcome:?}");
    };
    assert!(!installed.rolled_back);
    assert_eq!(installed.launch_exit_code, Some(0));
    assert_eq!(fs::read(&slot.current).unwrap(), incoming);
    assert_eq!(fs::read(&slot.previous).unwrap(), original);
    let (current, previous) = journal_versions(&slot.journal);
    assert_eq!(current, "0.2.0");
    assert_eq!(previous.as_deref(), Some("0.1.0"));
    assert_eq!(installed.journal.current_version, "0.2.0");
    assert_eq!(installed.journal.signer_status, "signed/ed25519");
}

#[test]
fn signed_feed_bad_signature_leaves_current_package() {
    let key = runtime_signing_key();
    let (file_name, original) = launcher(0, "N");
    let (_name, incoming) = launcher(0, "tampered-manifest");
    let (manifest, mut signature) = signed_manifest(&key, "0.2.0", "pkg", &file_name, &incoming);
    let last = signature.len() - 1;
    signature[last] ^= 0xff;
    let feed = LocalFeed::serve(routes(manifest, signature, &file_name, incoming));
    let slot = Slot::new(&original, &file_name);

    let err = install(&feed, &key, &slot, "0.1.0", "pkg", AppProductMode::Assist).unwrap_err();
    assert!(matches!(err, UpdateError::SignatureInvalid(_)), "{err}");
    assert_eq!(fs::read(&slot.current).unwrap(), original);
    assert!(!slot.previous.exists());
    assert!(!slot.journal.exists());
}

#[test]
fn signed_feed_hash_mismatch_leaves_current_package() {
    let key = runtime_signing_key();
    let (file_name, original) = launcher(0, "N");
    let (_name, advertised) = launcher(0, "advertised");
    let (_name, served) = launcher(0, "different-bytes");
    let (manifest, signature) = signed_manifest(&key, "0.2.0", "pkg", &file_name, &advertised);
    let feed = LocalFeed::serve(routes(manifest, signature, &file_name, served));
    let slot = Slot::new(&original, &file_name);

    let err = install(&feed, &key, &slot, "0.1.0", "pkg", AppProductMode::Assist).unwrap_err();
    assert!(matches!(err, UpdateError::HashMismatch { .. }), "{err}");
    assert_eq!(fs::read(&slot.current).unwrap(), original);
    assert!(!slot.previous.exists());
    assert!(!slot.journal.exists());
}

#[test]
fn signed_feed_failed_launch_rolls_back_to_previous() {
    let key = runtime_signing_key();
    let (file_name, original) = launcher(0, "N");
    let (_name, incoming) = launcher(1, "bad-launch");
    let (manifest, signature) = signed_manifest(&key, "0.2.0", "pkg", &file_name, &incoming);
    let feed = LocalFeed::serve(routes(manifest, signature, &file_name, incoming));
    let slot = Slot::new(&original, &file_name);

    let outcome = install(&feed, &key, &slot, "0.1.0", "pkg", AppProductMode::Assist).unwrap();
    let FetchInstallOutcome::Installed(installed) = outcome else {
        panic!("expected a rolled-back install, got {outcome:?}");
    };
    assert!(installed.rolled_back);
    assert_eq!(installed.launch_exit_code, Some(1));
    assert_eq!(fs::read(&slot.current).unwrap(), original);
    assert_eq!(fs::read(&slot.previous).unwrap(), original);
    let (current, previous) = journal_versions(&slot.journal);
    assert_eq!(current, "0.1.0");
    assert_eq!(previous.as_deref(), Some("0.2.0"));
    assert_eq!(installed.journal.current_version, "0.1.0");
}

#[test]
fn signed_feed_downgrade_does_not_replace_package() {
    let key = runtime_signing_key();
    let (file_name, original) = launcher(0, "current");
    let (_name, incoming) = launcher(0, "older");
    let (manifest, signature) = signed_manifest(&key, "0.1.0", "pkg", &file_name, &incoming);
    let feed = LocalFeed::serve(routes(manifest, signature, &file_name, incoming));
    let slot = Slot::new(&original, &file_name);

    let outcome = install(&feed, &key, &slot, "0.2.0", "pkg", AppProductMode::Assist).unwrap();
    assert!(matches!(outcome, FetchInstallOutcome::NoUpdate));
    assert_eq!(fs::read(&slot.current).unwrap(), original);
    assert!(!slot.previous.exists());
}

#[test]
fn signed_feed_parent_artifact_path_is_refused() {
    let key = runtime_signing_key();
    let (file_name, original) = launcher(0, "N");
    let payload = b"escape".to_vec();
    let (manifest, signature) = signed_manifest(&key, "0.2.0", "pkg", "../escape", &payload);
    let feed = LocalFeed::serve(routes(manifest, signature, "../escape", payload));
    let slot = Slot::new(&original, &file_name);

    let err = install(&feed, &key, &slot, "0.1.0", "pkg", AppProductMode::Assist).unwrap_err();
    assert!(matches!(err, UpdateError::UnsafePath(_)), "{err}");
    assert_eq!(fs::read(&slot.current).unwrap(), original);
    assert!(!slot.previous.exists());
}

struct CountingSource {
    hits: Arc<AtomicUsize>,
}

impl ManifestSource for CountingSource {
    fn fetch_manifest(&self) -> Result<(Vec<u8>, Option<Vec<u8>>), UpdateError> {
        self.hits.fetch_add(1, Ordering::SeqCst);
        Err(UpdateError::Feed("fetch was not allowed".to_string()))
    }
}

#[test]
fn manual_mode_never_polls_or_fetches() {
    let hits = Arc::new(AtomicUsize::new(0));
    let source = CountingSource {
        hits: Arc::clone(&hits),
    };
    let idle = policy("0.1.0");
    let skipped = Updater::new()
        .poll(AppProductMode::Manual, &source, &idle, None)
        .unwrap();
    assert!(matches!(skipped, UpdatePoll::SkippedManual));
    assert_eq!(hits.load(Ordering::SeqCst), 0);

    let fetched = Updater::new().poll(AppProductMode::Assist, &source, &idle, None);
    assert!(fetched.is_err());
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    let (file_name, original) = launcher(0, "N");
    let slot = Slot::new(&original, &file_name);
    let source = HttpManifestSource::with_timeout(
        format!("http://127.0.0.1:9/{MANIFEST_FILE}"),
        Duration::from_millis(200),
    )
    .unwrap();
    let outcome = Updater::new()
        .fetch_stage_swap_and_launch(HttpInstallRequest {
            mode: AppProductMode::Manual,
            source: &source,
            policy: &idle,
            verifying_key: &[7u8; 32],
            work_dir: &slot.work,
            artifact_name: "pkg",
            current_path: &slot.current,
            previous_path: &slot.previous,
            journal_path: &slot.journal,
            now_utc: "2026-10-08T00:00:00Z",
        })
        .unwrap();
    assert!(matches!(outcome, FetchInstallOutcome::SkippedManual));
    assert_eq!(fs::read(&slot.current).unwrap(), original);
}

#[test]
fn owner_feed_config_has_no_compiled_endpoint_or_key() {
    let empty = UpdateFeedConfig::parse(None, None).unwrap();
    assert!(empty.manifest_url.is_none());
    assert!(empty.verifying_key.is_none());

    let key_hex = "ab".repeat(32);
    let parsed = UpdateFeedConfig::parse(
        Some("https://updates.example.invalid/release-manifest.v1.toml"),
        Some(&key_hex),
    )
    .unwrap();
    assert_eq!(
        parsed.manifest_url.as_deref(),
        Some("https://updates.example.invalid/release-manifest.v1.toml")
    );
    assert_eq!(parsed.verifying_key.unwrap().len(), 32);

    assert!(
        UpdateFeedConfig::parse(
            Some("http://updates.example.invalid/release-manifest.v1.toml"),
            None
        )
        .is_err()
    );
    assert!(
        UpdateFeedConfig::parse(Some("http://127.0.0.1:9/release-manifest.v1.toml"), None).is_ok()
    );
    assert!(UpdateFeedConfig::parse(None, Some("abcd")).is_err());
    assert_eq!(
        UpdateFeedConfig::MANIFEST_URL_ENV,
        "LEGION_UPDATE_MANIFEST_URL"
    );
    assert_eq!(
        UpdateFeedConfig::VERIFYING_KEY_HEX_ENV,
        "LEGION_UPDATE_VERIFYING_KEY_HEX"
    );
}
