// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Manifest validation, approved-host policy, and verified artifact storage.
#![forbid(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// A validated external-artifact manifest.
///
/// Parsing rejects missing hashes, unsafe filenames, duplicate artifact names, and empty host
/// entries. Unknown JSON fields are retained only by the caller and have no effect here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Manifest schema understood by this build.
    pub schema_version: u32,
    /// Hostnames approved for artifact network access.
    pub allowed_hosts: AllowedHosts,
    /// External artifacts named by product code.
    pub artifacts: Vec<Artifact>,
}

impl Manifest {
    /// Parses and validates a manifest document.
    ///
    /// Returns [`ManifestError`] for malformed JSON or an invariant violation. Schema version zero,
    /// duplicate names, empty SHA-256 values, malformed SHA-256 values, and filenames containing a
    /// path separator are rejected.
    pub fn parse(json: &str) -> Result<Self, ManifestError> {
        let manifest: Self = serde_json::from_str(json).map_err(ManifestError::Json)?;
        if manifest.schema_version != 1 {
            return Err(ManifestError::InvalidSchemaVersion);
        }
        if manifest.allowed_hosts.0.is_empty()
            || manifest
                .allowed_hosts
                .0
                .iter()
                .any(|host| normalize_host(host).is_empty())
        {
            return Err(ManifestError::InvalidAllowedHost);
        }
        let mut names = std::collections::BTreeSet::new();
        for artifact in &manifest.artifacts {
            if artifact.name.trim().is_empty() {
                return Err(ManifestError::EmptyName);
            }
            if !names.insert(artifact.name.as_str()) {
                return Err(ManifestError::DuplicateName(artifact.name.clone()));
            }
            let hash = artifact.sha256.trim();
            if hash.is_empty() {
                return Err(ManifestError::EmptySha256(artifact.name.clone()));
            }
            if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(ManifestError::InvalidSha256(artifact.name.clone()));
            }
            if artifact.filename.is_empty()
                || artifact.filename.contains('/')
                || artifact.filename.contains('\\')
                || Path::new(&artifact.filename)
                    .file_name()
                    .and_then(|name| name.to_str())
                    != Some(artifact.filename.as_str())
            {
                return Err(ManifestError::UnsafeFilename(artifact.filename.clone()));
            }
        }
        Ok(manifest)
    }

    /// Reads and parses a manifest file.
    ///
    /// File-system failures and all [`Manifest::parse`] failures remain distinguishable.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ManifestError> {
        let json = fs::read_to_string(path).map_err(ManifestError::Io)?;
        Self::parse(&json)
    }
}

/// One manifest artifact and all provenance fields required by R2.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    /// Stable artifact identifier.
    pub name: String,
    /// Publisher version string.
    pub version: String,
    /// Safe leaf filename below the artifact store.
    pub filename: String,
    /// Publisher URL, checked again before every fetch.
    pub url: String,
    /// Expected byte length.
    pub size_bytes: u64,
    /// Expected lowercase or uppercase hexadecimal SHA-256.
    pub sha256: String,
    /// Publisher license description.
    pub license: String,
    /// Human-authored provenance note.
    pub provenance_note: String,
    /// Producer role such as `installer` or `builder`.
    pub fetched_by: String,
}

/// Approved hostnames used by [`AllowedHosts::permits`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AllowedHosts(pub Vec<String>);

impl AllowedHosts {
    /// Returns true only for an absolute HTTPS URL whose host equals an entry or is its subdomain.
    ///
    /// DNS names are ASCII case-folded and one or more trailing dots are ignored. A deceptive text
    /// suffix without a label boundary is rejected. Userinfo, malformed URLs, non-HTTPS schemes,
    /// and explicit non-default ports are rejected.
    pub fn permits(&self, url: &str) -> bool {
        let Some(host) = parse_https_host(url) else {
            return false;
        };
        self.0.iter().any(|allowed| {
            let allowed = normalize_host(allowed);
            !allowed.is_empty()
                && (host == allowed
                    || host
                        .strip_suffix(&allowed)
                        .is_some_and(|prefix| prefix.ends_with('.')))
        })
    }
}

fn normalize_host(host: &str) -> String {
    host.trim().trim_end_matches('.').to_ascii_lowercase()
}

fn parse_https_host(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://").or_else(|| {
        url.get(..8)
            .filter(|prefix| prefix.eq_ignore_ascii_case("https://"))
            .and_then(|_| url.get(8..))
    })?;
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let host = if authority.starts_with('[') {
        return None;
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        if port != "443" || host.contains(':') {
            return None;
        }
        host
    } else {
        authority
    };
    let normalized = normalize_host(host);
    (!normalized.is_empty()
        && normalized
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-')))
    .then_some(normalized)
}

/// Errors while parsing or validating an artifact manifest.
#[derive(Debug, Error)]
pub enum ManifestError {
    /// The JSON document is malformed or has the wrong field types.
    #[error("manifest JSON is invalid")]
    Json(#[source] serde_json::Error),
    /// The manifest could not be read.
    #[error("manifest could not be read")]
    Io(#[source] io::Error),
    /// Only schema version 1 is supported.
    #[error("manifest schema version is invalid")]
    InvalidSchemaVersion,
    /// At least one allowed host is empty or no hosts are configured.
    #[error("manifest allowed host is invalid")]
    InvalidAllowedHost,
    /// An artifact has no name.
    #[error("artifact name is empty")]
    EmptyName,
    /// Artifact names must be unique.
    #[error("artifact name is duplicated: {0}")]
    DuplicateName(String),
    /// An artifact hash is empty.
    #[error("artifact has no SHA-256: {0}")]
    EmptySha256(String),
    /// An artifact hash is not exactly 64 hexadecimal digits.
    #[error("artifact SHA-256 is malformed: {0}")]
    InvalidSha256(String),
    /// An artifact filename is not a leaf name.
    #[error("artifact filename contains a directory: {0}")]
    UnsafeFilename(String),
}

/// Result metadata from an HTTP transfer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchOutcome {
    /// HTTP status code returned after redirects.
    pub status: u16,
    /// Final response URL after redirects, when the adapter can report it.
    pub final_url: Option<String>,
    /// Number of response bytes written to the sink.
    pub bytes_written: u64,
}

/// Network adapter used by the artifact store.
pub trait HttpFetch: Send + Sync {
    /// Fetches `url`, optionally requesting bytes starting at `range_start`.
    ///
    /// Implementations write only response-body bytes to `sink`, call `progress` with each positive
    /// byte increment, follow ordinary GET redirects, and return status 200 or 206. Other statuses,
    /// transport errors, and sink failures return [`FetchError`].
    fn fetch(
        &self,
        url: &str,
        range_start: Option<u64>,
        sink: &mut dyn Write,
        progress: &mut dyn FnMut(u64) -> bool,
    ) -> Result<FetchOutcome, FetchError>;
}

impl<T> HttpFetch for std::sync::Arc<T>
where
    T: HttpFetch + ?Sized,
{
    fn fetch(
        &self,
        url: &str,
        range_start: Option<u64>,
        sink: &mut dyn Write,
        progress: &mut dyn FnMut(u64) -> bool,
    ) -> Result<FetchOutcome, FetchError> {
        (**self).fetch(url, range_start, sink, progress)
    }
}

/// Errors reported by an [`HttpFetch`] implementation.
#[derive(Debug, Error)]
pub enum FetchError {
    /// The HTTP client or server rejected the request.
    #[error("HTTP request failed: {0}")]
    Http(String),
    /// The response body could not be copied to storage.
    #[error("HTTP response could not be written")]
    Io(#[source] io::Error),
    /// The caller cancelled the transfer after a completed chunk.
    #[error("HTTP request was cancelled")]
    Cancelled,
    /// The server returned a status incompatible with the requested transfer mode.
    #[error("unexpected HTTP status {status} for resume={resuming}")]
    UnexpectedStatus {
        /// Received response status.
        status: u16,
        /// Whether a byte range was requested.
        resuming: bool,
    },
}

/// Production HTTP adapter backed by `ureq` with rustls.
///
/// It follows at most ten redirects and reports the final URL so [`ArtifactStore`] can apply the
/// manifest host policy after redirection. Ureq also rejects non-HTTPS redirect targets.
#[cfg(feature = "real-http")]
#[derive(Debug)]
pub struct UreqFetch {
    agent: ureq::Agent,
}

#[cfg(feature = "real-http")]
impl Default for UreqFetch {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "real-http")]
impl UreqFetch {
    /// Creates the production client with HTTPS-only requests and bounded redirects.
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .https_only(true)
            .max_redirects(10)
            .user_agent("Open-Mobile-Emulator")
            .build();
        Self {
            agent: ureq::Agent::new_with_config(config),
        }
    }
}

#[cfg(feature = "real-http")]
impl HttpFetch for UreqFetch {
    fn fetch(
        &self,
        url: &str,
        range_start: Option<u64>,
        sink: &mut dyn Write,
        progress: &mut dyn FnMut(u64) -> bool,
    ) -> Result<FetchOutcome, FetchError> {
        use ureq::ResponseExt;

        let mut request = self.agent.get(url);
        if let Some(start) = range_start {
            request = request.header("Range", format!("bytes={start}-"));
        }
        let mut response = request
            .call()
            .map_err(|error| FetchError::Http(error.to_string()))?;
        let status = response.status().as_u16();
        let final_url = response.get_uri().to_string();
        let valid = match range_start {
            Some(_) => matches!(status, 200 | 206),
            None => status == 200,
        };
        if !valid {
            return Err(FetchError::UnexpectedStatus {
                status,
                resuming: range_start.is_some(),
            });
        }
        let mut reader = response.body_mut().as_reader();
        let mut buffer = [0_u8; 64 * 1024];
        let mut bytes_written = 0_u64;
        loop {
            let count = reader.read(&mut buffer).map_err(FetchError::Io)?;
            if count == 0 {
                break;
            }
            sink.write_all(&buffer[..count]).map_err(FetchError::Io)?;
            let count = u64::try_from(count).expect("buffer length fits u64");
            bytes_written += count;
            if !progress(count) {
                return Err(FetchError::Cancelled);
            }
        }
        Ok(FetchOutcome {
            status,
            final_url: Some(final_url),
            bytes_written,
        })
    }
}

/// Progress boundary reported by a cancellable artifact operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreProgress {
    /// A response chunk was written to the partial file.
    Bytes(u64),
    /// Streaming SHA-256 verification is about to run.
    Verifying,
}

/// Verification state of one named artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verification {
    /// The complete file has the expected size and digest.
    Verified,
    /// No complete file exists.
    Missing,
    /// A complete file exists but violates the stated invariant.
    Mismatch(String),
}

/// A complete verified artifact file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedFile {
    /// Artifact name from the manifest.
    pub name: String,
    /// Complete local path after atomic promotion.
    pub path: PathBuf,
    /// Verified byte length.
    pub size_bytes: u64,
}

/// Verified storage rooted in one caller-chosen directory.
#[derive(Clone)]
pub struct ArtifactStore {
    manifest: Manifest,
    dir: PathBuf,
    http: std::sync::Arc<dyn HttpFetch>,
}

impl std::fmt::Debug for ArtifactStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ArtifactStore")
            .field("manifest", &self.manifest)
            .field("dir", &self.dir)
            .finish_non_exhaustive()
    }
}

impl ArtifactStore {
    /// Creates a store without touching the file system.
    ///
    /// The supplied manifest is assumed to have come from [`Manifest::parse`]. Each operation still
    /// enforces safe filenames and the allowed-host policy before file or network mutation.
    pub fn new(manifest: Manifest, dir: impl Into<PathBuf>, http: Box<dyn HttpFetch>) -> Self {
        Self {
            manifest,
            dir: dir.into(),
            http: http.into(),
        }
    }

    /// Returns the first artifact whose producer role is exactly `installer`.
    pub fn installer(&self) -> Option<&Artifact> {
        self.manifest
            .artifacts
            .iter()
            .find(|artifact| artifact.fetched_by == "installer")
    }

    /// Returns trusted metadata for one named artifact.
    pub fn metadata(&self, name: &str) -> Option<&Artifact> {
        self.artifact(name)
    }

    /// Returns a verified file descriptor without starting network work.
    pub fn verified(&self, name: &str) -> Option<VerifiedFile> {
        let artifact = self.artifact(name)?;
        let path = self.dir.join(&artifact.filename);
        matches!(self.verify_path(artifact, &path), Verification::Verified)
            .then(|| verified_file(artifact, path))
    }

    /// Returns bytes retained in the resumable partial file.
    pub fn partial_len(&self, name: &str) -> u64 {
        self.artifact(name)
            .and_then(|artifact| {
                fs::metadata(self.dir.join(format!("{}.part", artifact.filename))).ok()
            })
            .filter(|metadata| metadata.is_file())
            .map_or(0, |metadata| metadata.len())
    }

    /// Verifies a complete named file by size first and streaming SHA-256 second.
    ///
    /// Missing names and read errors are represented as mismatches without exposing OS error text.
    pub fn verify(&self, name: &str) -> Verification {
        let Some(artifact) = self.artifact(name) else {
            return Verification::Mismatch("artifact is not in the manifest".to_owned());
        };
        self.verify_path(artifact, &self.dir.join(&artifact.filename))
    }

    /// Ensures a named artifact is present and verified.
    ///
    /// A completed invalid file is deleted before transfer. Downloads append to `<filename>.part`;
    /// a server that ignores a range request causes a safe restart from byte zero. The partial file
    /// remains after transport or verification failure so a later call can resume. A verified partial
    /// is atomically renamed to the final leaf path.
    pub fn ensure(
        &self,
        name: &str,
        progress: &mut dyn FnMut(u64),
    ) -> Result<VerifiedFile, StoreError> {
        self.ensure_cancellable(name, &mut |event| {
            if let StoreProgress::Bytes(bytes) = event {
                progress(bytes);
            }
            true
        })
    }

    /// Ensures an artifact while allowing the caller to stop at transfer and verification
    /// boundaries. A cancelled partial file is retained for the next resume attempt.
    pub fn ensure_cancellable(
        &self,
        name: &str,
        progress: &mut dyn FnMut(StoreProgress) -> bool,
    ) -> Result<VerifiedFile, StoreError> {
        let artifact = self
            .artifact(name)
            .ok_or_else(|| StoreError::UnknownArtifact(name.to_owned()))?;
        if !self.manifest.allowed_hosts.permits(&artifact.url) {
            return Err(StoreError::UrlNotAllowed);
        }
        fs::create_dir_all(&self.dir).map_err(StoreError::Io)?;
        let final_path = self.dir.join(&artifact.filename);
        if final_path.is_file() {
            if !progress(StoreProgress::Verifying) {
                return Err(StoreError::Cancelled);
            }
            match self.verify_path(artifact, &final_path) {
                Verification::Verified => return Ok(verified_file(artifact, final_path)),
                Verification::Missing => {}
                Verification::Mismatch(_) => {
                    fs::remove_file(&final_path).map_err(StoreError::Io)?
                }
            }
        }

        let part_path = self.dir.join(format!("{}.part", artifact.filename));
        let mut resume = match fs::metadata(&part_path) {
            Ok(metadata) if metadata.is_file() && metadata.len() <= artifact.size_bytes => {
                metadata.len()
            }
            Ok(_) => {
                fs::remove_file(&part_path).map_err(StoreError::Io)?;
                0
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
            Err(error) => return Err(StoreError::Io(error)),
        };
        if resume == artifact.size_bytes {
            if !progress(StoreProgress::Verifying) {
                return Err(StoreError::Cancelled);
            }
            match self.verify_path(artifact, &part_path) {
                Verification::Verified => {
                    promote_partial(&part_path, &final_path)?;
                    return Ok(verified_file(artifact, final_path));
                }
                Verification::Missing | Verification::Mismatch(_) => {
                    fs::remove_file(&part_path).map_err(StoreError::Io)?;
                    resume = 0;
                }
            }
        }
        let mut file = open_partial(&part_path, resume > 0)?;
        let outcome = self
            .http
            .fetch(
                &artifact.url,
                (resume > 0).then_some(resume),
                &mut file,
                &mut |bytes| progress(StoreProgress::Bytes(bytes)),
            )
            .map_err(StoreError::Fetch)?;
        self.verify_fetch_url(&outcome)?;
        if resume > 0 && outcome.status == 200 {
            drop(file);
            file = open_partial(&part_path, false)?;
            resume = 0;
            let restarted = self
                .http
                .fetch(&artifact.url, None, &mut file, &mut |bytes| {
                    progress(StoreProgress::Bytes(bytes))
                })
                .map_err(StoreError::Fetch)?;
            self.verify_fetch_url(&restarted)?;
        } else if resume > 0 && outcome.status == 206 {
            let expected_content = artifact.size_bytes - resume;
            if outcome.bytes_written != expected_content {
                return Err(StoreError::VerificationFailed(format!(
                    "range response size {}, expected {expected_content}",
                    outcome.bytes_written
                )));
            }
        }
        file.sync_all().map_err(StoreError::Io)?;
        drop(file);

        if !progress(StoreProgress::Verifying) {
            return Err(StoreError::Cancelled);
        }
        match self.verify_path(artifact, &part_path) {
            Verification::Verified => {
                promote_partial(&part_path, &final_path)?;
                Ok(verified_file(artifact, final_path))
            }
            Verification::Missing => Err(StoreError::VerificationFailed(
                "partial file disappeared".to_owned(),
            )),
            Verification::Mismatch(reason) => {
                let downloaded = fs::metadata(&part_path)
                    .map(|metadata| metadata.len())
                    .unwrap_or(resume);
                Err(StoreError::VerificationFailed(format!(
                    "{reason}; received {downloaded} bytes"
                )))
            }
        }
    }

    fn verify_fetch_url(&self, outcome: &FetchOutcome) -> Result<(), StoreError> {
        if outcome
            .final_url
            .as_deref()
            .is_some_and(|url| !self.manifest.allowed_hosts.permits(url))
        {
            Err(StoreError::UrlNotAllowed)
        } else {
            Ok(())
        }
    }

    fn artifact(&self, name: &str) -> Option<&Artifact> {
        self.manifest
            .artifacts
            .iter()
            .find(|artifact| artifact.name == name)
    }

    fn verify_path(&self, artifact: &Artifact, path: &Path) -> Verification {
        let metadata = match fs::metadata(path) {
            Ok(metadata) if metadata.is_file() => metadata,
            Ok(_) => return Verification::Mismatch("path is not a regular file".to_owned()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Verification::Missing,
            Err(_) => return Verification::Mismatch("file metadata is unavailable".to_owned()),
        };
        if metadata.len() != artifact.size_bytes {
            return Verification::Mismatch(format!(
                "size {}, expected {}",
                metadata.len(),
                artifact.size_bytes
            ));
        }
        let marker = marker_path(path);
        let stamp = marker_stamp(artifact, &metadata);
        if let Some(stamp) = &stamp
            && fs::read_to_string(&marker).is_ok_and(|recorded| recorded == *stamp)
        {
            return Verification::Verified;
        }
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(_) => return Verification::Mismatch("file cannot be read".to_owned()),
        };
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            match file.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => hasher.update(&buffer[..count]),
                Err(_) => return Verification::Mismatch("file cannot be read".to_owned()),
            }
        }
        let actual = format!("{:x}", hasher.finalize());
        if actual.eq_ignore_ascii_case(artifact.sha256.trim()) {
            if let Some(stamp) = stamp {
                // Best effort: a marker that cannot be written only costs the next digest.
                let _ = fs::write(&marker, stamp);
            }
            Verification::Verified
        } else {
            let _ = fs::remove_file(&marker);
            Verification::Mismatch(format!(
                "SHA-256 mismatch; expected {}",
                artifact.sha256.trim().to_ascii_lowercase()
            ))
        }
    }
}

/// Path of the verification marker kept beside a complete artifact file.
fn marker_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".verified");
    PathBuf::from(name)
}

/// The cached proof of one completed SHA-256 check: the manifest digest, the file size and the
/// modification time. Digesting a multi-gigabyte image takes seconds and the runtime reads the
/// verification state on every snapshot, so the marker stands in for the digest while the size and
/// modification time hold. A new manifest digest or any write to the file invalidates it.
fn marker_stamp(artifact: &Artifact, metadata: &fs::Metadata) -> Option<String> {
    let modified = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    Some(format!(
        "{}\n{}\n{}.{:09}\n",
        artifact.sha256.trim().to_ascii_lowercase(),
        metadata.len(),
        modified.as_secs(),
        modified.subsec_nanos()
    ))
}

/// Renames a verified partial to its final name and carries its marker along (best effort).
fn promote_partial(part_path: &Path, final_path: &Path) -> Result<(), StoreError> {
    fs::rename(part_path, final_path).map_err(StoreError::Io)?;
    let _ = fs::rename(marker_path(part_path), marker_path(final_path));
    Ok(())
}

fn open_partial(path: &Path, append: bool) -> Result<File, StoreError> {
    OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(path)
        .map_err(StoreError::Io)
}

fn verified_file(artifact: &Artifact, path: PathBuf) -> VerifiedFile {
    VerifiedFile {
        name: artifact.name.clone(),
        path,
        size_bytes: artifact.size_bytes,
    }
}

/// Errors while ensuring an artifact is locally verified.
#[derive(Debug, Error)]
pub enum StoreError {
    /// The requested name is absent from the manifest.
    #[error("artifact is not in the manifest: {0}")]
    UnknownArtifact(String),
    /// The manifest URL does not satisfy the approved-host policy.
    #[error("artifact URL is not allowed")]
    UrlNotAllowed,
    /// Local storage could not be read or changed.
    #[error("artifact storage failed")]
    Io(#[source] io::Error),
    /// Network transfer failed; an existing partial file is retained.
    #[error("artifact transfer failed")]
    Fetch(#[source] FetchError),
    /// Downloaded bytes do not match the manifest; the partial file is retained.
    #[error("artifact verification failed: {0}")]
    VerificationFailed(String),
    /// The caller cancelled and any partial file remains available for resume.
    #[error("artifact transfer was cancelled")]
    Cancelled,
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Clone, Debug)]
    struct FakeFetch {
        body: Vec<u8>,
        calls: Arc<Mutex<Vec<Option<u64>>>>,
        ignore_range: bool,
        final_url: Option<String>,
    }

    impl HttpFetch for FakeFetch {
        fn fetch(
            &self,
            _url: &str,
            range_start: Option<u64>,
            sink: &mut dyn Write,
            progress: &mut dyn FnMut(u64) -> bool,
        ) -> Result<FetchOutcome, FetchError> {
            self.calls.lock().expect("calls lock").push(range_start);
            let (status, start) = if self.ignore_range && range_start.is_some() {
                (200, 0)
            } else if let Some(start) = range_start {
                (206, usize::try_from(start).expect("test offset fits usize"))
            } else {
                (200, 0)
            };
            let bytes = &self.body[start..];
            sink.write_all(bytes).map_err(FetchError::Io)?;
            if !progress(u64::try_from(bytes.len()).expect("test length fits u64")) {
                return Err(FetchError::Cancelled);
            }
            Ok(FetchOutcome {
                status,
                final_url: self.final_url.clone(),
                bytes_written: u64::try_from(bytes.len()).expect("test length fits u64"),
            })
        }
    }

    fn manifest(body: &[u8]) -> Manifest {
        let hash = format!("{:x}", Sha256::digest(body));
        Manifest::parse(&format!(
            r#"{{
                "schema_version": 1,
                "allowed_hosts": ["sourceforge.net"],
                "artifacts": [{{
                    "name": "guest-iso",
                    "version": "1",
                    "filename": "guest.iso",
                    "url": "https://downloads.sourceforge.net/guest.iso",
                    "size_bytes": {},
                    "sha256": "{hash}",
                    "license": "mixed",
                    "provenance_note": "publisher",
                    "fetched_by": "installer"
                }}]
            }}"#,
            body.len()
        ))
        .expect("manifest parses")
    }

    #[test]
    fn manifest_accepts_repository_shape_and_rejects_unsafe_fields() {
        let valid = manifest(b"complete");
        assert_eq!(valid.installer().name, "guest-iso");

        let empty_hash = serde_json::to_string(&valid)
            .expect("serialize")
            .replace(&valid.artifacts[0].sha256, "");
        assert!(matches!(
            Manifest::parse(&empty_hash),
            Err(ManifestError::EmptySha256(_))
        ));
        let unsupported = serde_json::to_string(&valid)
            .expect("serialize")
            .replace("\"schema_version\":1", "\"schema_version\":2");
        assert!(matches!(
            Manifest::parse(&unsupported),
            Err(ManifestError::InvalidSchemaVersion)
        ));
        let unsafe_name = serde_json::to_string(&valid)
            .expect("serialize")
            .replace("guest.iso", "dir/guest.iso");
        assert!(matches!(
            Manifest::parse(&unsafe_name),
            Err(ManifestError::UnsafeFilename(_))
        ));
    }

    trait InstallerArtifact {
        fn installer(&self) -> &Artifact;
    }

    impl InstallerArtifact for Manifest {
        fn installer(&self) -> &Artifact {
            self.artifacts
                .iter()
                .find(|artifact| artifact.fetched_by == "installer")
                .expect("installer artifact")
        }
    }

    #[test]
    fn allowed_hosts_match_only_https_label_suffixes() {
        let hosts = AllowedHosts(vec!["SourceForge.NET.".to_owned()]);
        assert!(hosts.permits("https://downloads.sourceforge.net/x"));
        assert!(hosts.permits("https://phoenixnap.dl.sourceforge.net/x"));
        assert!(!hosts.permits("https://evil.example.com/x"));
        assert!(!hosts.permits("https://sourceforge.net.evil.com/x"));
        assert!(!hosts.permits("https://notsourceforge.net/x"));
        assert!(!hosts.permits("http://downloads.sourceforge.net/x"));
        assert!(hosts.permits("https://downloads.sourceforge.net:443/x"));
        assert!(!hosts.permits("https://downloads.sourceforge.net:444/x"));
    }

    #[test]
    fn verifies_size_before_hash() {
        let directory = tempfile::tempdir().expect("temp directory");
        fs::write(directory.path().join("guest.iso"), b"wrong").expect("write file");
        let store = ArtifactStore::new(
            manifest(b"complete"),
            directory.path(),
            Box::new(FakeFetch {
                body: Vec::new(),
                calls: Arc::new(Mutex::new(Vec::new())),
                ignore_range: false,
                final_url: None,
            }),
        );
        assert_eq!(
            store.verify("guest-iso"),
            Verification::Mismatch("size 5, expected 8".to_owned())
        );
    }

    #[test]
    fn verification_marker_stands_in_for_the_digest_while_size_and_mtime_hold() {
        let directory = tempfile::tempdir().expect("temp directory");
        let path = directory.path().join("guest.iso");
        fs::write(&path, b"complete").expect("write file");
        let store = ArtifactStore::new(
            manifest(b"complete"),
            directory.path(),
            Box::new(FakeFetch {
                body: Vec::new(),
                calls: Arc::new(Mutex::new(Vec::new())),
                ignore_range: false,
                final_url: None,
            }),
        );
        assert_eq!(store.verify("guest-iso"), Verification::Verified);
        let marker = directory.path().join("guest.iso.verified");
        let recorded = fs::read_to_string(&marker).expect("marker written");
        let digest = format!("{:x}", Sha256::digest(b"complete"));
        assert!(
            recorded.starts_with(&format!("{digest}\n8\n")),
            "{recorded}"
        );
        assert_eq!(recorded.lines().count(), 3);

        // Same size, other bytes, original modification time: the marker still answers.
        let modified = fs::metadata(&path)
            .expect("metadata")
            .modified()
            .expect("modification time");
        fs::write(&path, b"complet3").expect("rewrite same size");
        File::options()
            .write(true)
            .open(&path)
            .expect("reopen")
            .set_modified(modified)
            .expect("restore modification time");
        assert_eq!(store.verify("guest-iso"), Verification::Verified);

        // A later write (new modification time) forces the digest again and drops the marker.
        File::options()
            .write(true)
            .open(&path)
            .expect("reopen")
            .set_modified(modified + std::time::Duration::from_secs(5))
            .expect("advance modification time");
        assert!(matches!(
            store.verify("guest-iso"),
            Verification::Mismatch(reason) if reason.starts_with("SHA-256 mismatch")
        ));
        assert!(!marker.exists());
    }

    #[test]
    fn promotion_carries_the_verification_marker() {
        let body = b"complete".to_vec();
        let directory = tempfile::tempdir().expect("temp directory");
        let store = ArtifactStore::new(
            manifest(&body),
            directory.path(),
            Box::new(FakeFetch {
                body: body.clone(),
                calls: Arc::new(Mutex::new(Vec::new())),
                ignore_range: false,
                final_url: None,
            }),
        );
        store
            .ensure("guest-iso", &mut |_| {})
            .expect("ensure artifact");
        assert!(directory.path().join("guest.iso.verified").is_file());
        assert!(!directory.path().join("guest.iso.part.verified").exists());
        assert_eq!(store.verify("guest-iso"), Verification::Verified);
    }

    #[test]
    fn downloads_verifies_and_atomically_promotes() {
        let body = b"complete".to_vec();
        let directory = tempfile::tempdir().expect("temp directory");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let store = ArtifactStore::new(
            manifest(&body),
            directory.path(),
            Box::new(FakeFetch {
                body: body.clone(),
                calls: Arc::clone(&calls),
                ignore_range: false,
                final_url: None,
            }),
        );
        let mut progress = 0;
        let file = store
            .ensure("guest-iso", &mut |count| progress += count)
            .expect("ensure artifact");
        assert_eq!(fs::read(file.path).expect("read complete"), body);
        assert!(!directory.path().join("guest.iso.part").exists());
        assert_eq!(*calls.lock().expect("calls lock"), vec![None]);
        assert_eq!(progress, 8);
    }

    #[test]
    fn promotes_complete_verified_partial_without_network() {
        let body = b"complete".to_vec();
        let directory = tempfile::tempdir().expect("temp directory");
        fs::write(directory.path().join("guest.iso.part"), &body).expect("complete partial");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let store = ArtifactStore::new(
            manifest(&body),
            directory.path(),
            Box::new(FakeFetch {
                body: body.clone(),
                calls: Arc::clone(&calls),
                ignore_range: false,
                final_url: None,
            }),
        );
        store
            .ensure("guest-iso", &mut |_| {})
            .expect("promote artifact");
        assert!(calls.lock().expect("calls lock").is_empty());
        assert_eq!(
            fs::read(directory.path().join("guest.iso")).expect("read file"),
            body
        );
    }

    #[test]
    fn cancellation_retains_partial_for_resume() {
        let body = b"complete".to_vec();
        let directory = tempfile::tempdir().expect("temp directory");
        let store = ArtifactStore::new(
            manifest(&body),
            directory.path(),
            Box::new(FakeFetch {
                body: body.clone(),
                calls: Arc::new(Mutex::new(Vec::new())),
                ignore_range: false,
                final_url: None,
            }),
        );
        let result = store.ensure_cancellable("guest-iso", &mut |event| {
            !matches!(event, StoreProgress::Bytes(_))
        });
        assert!(matches!(
            result,
            Err(StoreError::Fetch(FetchError::Cancelled))
        ));
        assert_eq!(
            fs::read(directory.path().join("guest.iso.part")).expect("retained partial"),
            body
        );
    }

    #[test]
    fn resumes_from_existing_partial_file() {
        let body = b"complete".to_vec();
        let directory = tempfile::tempdir().expect("temp directory");
        fs::write(directory.path().join("guest.iso.part"), b"comp").expect("partial file");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let store = ArtifactStore::new(
            manifest(&body),
            directory.path(),
            Box::new(FakeFetch {
                body: body.clone(),
                calls: Arc::clone(&calls),
                ignore_range: false,
                final_url: None,
            }),
        );
        store
            .ensure("guest-iso", &mut |_| {})
            .expect("resume artifact");
        assert_eq!(*calls.lock().expect("calls lock"), vec![Some(4)]);
        assert_eq!(
            fs::read(directory.path().join("guest.iso")).expect("read file"),
            body
        );
    }

    #[test]
    fn restarts_when_server_ignores_range() {
        let body = b"complete".to_vec();
        let directory = tempfile::tempdir().expect("temp directory");
        fs::write(directory.path().join("guest.iso.part"), b"comp").expect("partial file");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let store = ArtifactStore::new(
            manifest(&body),
            directory.path(),
            Box::new(FakeFetch {
                body: body.clone(),
                calls: Arc::clone(&calls),
                ignore_range: true,
                final_url: None,
            }),
        );
        store
            .ensure("guest-iso", &mut |_| {})
            .expect("restart artifact");
        assert_eq!(*calls.lock().expect("calls lock"), vec![Some(4), None]);
        assert_eq!(
            fs::read(directory.path().join("guest.iso")).expect("read file"),
            body
        );
    }

    #[test]
    fn removes_bad_complete_but_keeps_bad_partial() {
        let body = b"complete".to_vec();
        let directory = tempfile::tempdir().expect("temp directory");
        let final_path = directory.path().join("guest.iso");
        fs::write(&final_path, b"bad data").expect("bad complete");
        let store = ArtifactStore::new(
            manifest(&body),
            directory.path(),
            Box::new(FakeFetch {
                body: b"bad data".to_vec(),
                calls: Arc::new(Mutex::new(Vec::new())),
                ignore_range: false,
                final_url: None,
            }),
        );
        let result = store.ensure("guest-iso", &mut |_| {});
        assert!(matches!(result, Err(StoreError::VerificationFailed(_))));
        assert!(!final_path.exists());
        assert!(directory.path().join("guest.iso.part").exists());
    }

    #[test]
    fn refuses_unapproved_redirect_after_fetch() {
        let body = b"complete".to_vec();
        let directory = tempfile::tempdir().expect("temp directory");
        let store = ArtifactStore::new(
            manifest(&body),
            directory.path(),
            Box::new(FakeFetch {
                body,
                calls: Arc::new(Mutex::new(Vec::new())),
                ignore_range: false,
                final_url: Some("https://evil.example/guest.iso".to_owned()),
            }),
        );
        assert!(matches!(
            store.ensure("guest-iso", &mut |_| {}),
            Err(StoreError::UrlNotAllowed)
        ));
        assert!(directory.path().join("guest.iso.part").exists());
    }

    #[test]
    fn refuses_unapproved_url_without_fetching() {
        let mut value = manifest(b"complete");
        value.artifacts[0].url = "https://sourceforge.net.evil.example/guest.iso".to_owned();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let directory = tempfile::tempdir().expect("temp directory");
        let store = ArtifactStore::new(
            value,
            directory.path(),
            Box::new(FakeFetch {
                body: b"complete".to_vec(),
                calls: Arc::clone(&calls),
                ignore_range: false,
                final_url: None,
            }),
        );
        assert!(matches!(
            store.ensure("guest-iso", &mut |_| {}),
            Err(StoreError::UrlNotAllowed)
        ));
        assert!(calls.lock().expect("calls lock").is_empty());
    }
}
