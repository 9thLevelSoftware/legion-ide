use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use legion_app::language::{
    ArtifactDescriptor, LanguageArtifactMaterializer, MaterializeError, MaterializeRequest,
};
use legion_lsp::LspArtifactRuntime;
use legion_protocol::{CausalityId, CorrelationId};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;
use tar::{Builder, Header};
use tempfile::tempdir;
use uuid::Uuid;

fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
    {
        let mut tar = Builder::new(&mut gzip);
        for (name, body) in entries {
            let mut header = Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, *name, *body).unwrap();
        }
        tar.finish().unwrap();
    }
    gzip.finish().unwrap()
}

fn pax_archive(path: &str, body: &[u8]) -> Vec<u8> {
    let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
    {
        let mut tar = Builder::new(&mut gzip);
        tar.append_pax_extensions([("path", path.as_bytes())])
            .unwrap();
        let mut header = Header::new_ustar();
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, "placeholder", body).unwrap();
        let files = [
            (
                "package/package.json",
                br#"{"name":"pyright","version":"1.1.400"}"#.as_slice(),
            ),
            ("package/langserver.index.js", b"entry".as_slice()),
        ];
        for (name, body) in files {
            let mut header = Header::new_ustar();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, name, body).unwrap();
        }
        tar.finish().unwrap();
    }
    gzip.finish().unwrap()
}

fn raw_unsafe_archive(name: &[u8], body: &[u8]) -> Vec<u8> {
    raw_archive(&[(name, body, b'0')])
}

fn raw_archive(entries: &[(&[u8], &[u8], u8)]) -> Vec<u8> {
    let mut tar = Vec::new();
    for (name, body, typ) in entries {
        let mut raw = vec![0u8; 512];
        raw[..name.len()].copy_from_slice(name);
        raw[100..108].copy_from_slice(b"0000644\0");
        let size = format!("{:011o}\0", body.len());
        raw[124..136].copy_from_slice(size.as_bytes());
        raw[148..156].fill(b' ');
        raw[156] = *typ;
        raw[257..263].copy_from_slice(b"ustar\0");
        let sum: u32 = raw.iter().map(|b| *b as u32).sum();
        let checksum = format!("{:06o}\0 ", sum);
        raw[148..156].copy_from_slice(checksum.as_bytes());
        tar.extend_from_slice(&raw);
        tar.extend_from_slice(body);
        tar.resize(tar.len() + ((512 - body.len() % 512) % 512), 0);
    }
    tar.resize(tar.len() + 1024, 0);
    let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
    gzip.write_all(&tar).unwrap();
    gzip.finish().unwrap()
}

fn raw_pax_size_override_archive(effective_size: u64) -> Vec<u8> {
    let mut entries = Vec::new();
    let pax = pax_record("size", &effective_size.to_string());
    entries.push((pax, b'x'));
    let mut tar = Vec::new();
    for (body, typ) in entries {
        append_raw_header(&mut tar, b"pax", body.len() as u64, &body, typ);
    }
    append_raw_header(&mut tar, b"package/data", 5, b"hello", b'0');
    append_raw_header(
        &mut tar,
        b"package/package.json",
        br#"{"name":"pyright","version":"1.1.400"}"#.len() as u64,
        br#"{"name":"pyright","version":"1.1.400"}"#,
        b'0',
    );
    append_raw_header(&mut tar, b"package/langserver.index.js", 5, b"entry", b'0');
    tar.resize(tar.len() + 1024, 0);
    let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
    gzip.write_all(&tar).unwrap();
    gzip.finish().unwrap()
}

fn raw_gnu_long_and_local_pax_archive(gnu_first: bool) -> Vec<u8> {
    let mut tar = Vec::new();
    let gnu_path = b"package/gnu-long\0";
    let pax = pax_record("path", "package/pax-long");
    let mut append_metadata = |typ: u8, body: &[u8]| {
        append_raw_header(&mut tar, b"metadata", body.len() as u64, body, typ);
    };
    if gnu_first {
        append_metadata(b'L', gnu_path);
        append_metadata(b'x', &pax);
    } else {
        append_metadata(b'x', &pax);
        append_metadata(b'L', gnu_path);
    }
    append_raw_header(&mut tar, b"placeholder", 5, b"hello", b'0');
    append_raw_header(
        &mut tar,
        b"package/package.json",
        br#"{"name":"pyright","version":"1.1.400"}"#.len() as u64,
        br#"{"name":"pyright","version":"1.1.400"}"#,
        b'0',
    );
    append_raw_header(&mut tar, b"package/langserver.index.js", 5, b"entry", b'0');
    tar.resize(tar.len() + 1024, 0);
    let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
    gzip.write_all(&tar).unwrap();
    gzip.finish().unwrap()
}

fn append_raw_header(tar: &mut Vec<u8>, name: &[u8], declared_size: u64, body: &[u8], typ: u8) {
    let mut raw = vec![0u8; 512];
    raw[..name.len()].copy_from_slice(name);
    raw[100..108].copy_from_slice(b"0000644\0");
    let size = format!("{:011o}\0", declared_size);
    raw[124..136].copy_from_slice(size.as_bytes());
    raw[148..156].fill(b' ');
    raw[156] = typ;
    raw[257..263].copy_from_slice(b"ustar\0");
    raw[263..265].copy_from_slice(b"00");
    raw[263..265].copy_from_slice(b"00");
    let sum: u32 = raw.iter().map(|b| *b as u32).sum();
    raw[148..156].copy_from_slice(format!("{:06o}\0 ", sum).as_bytes());
    tar.extend_from_slice(&raw);
    tar.extend_from_slice(body);
    tar.resize(tar.len() + ((512 - body.len() % 512) % 512), 0);
}

fn pax_record(key: &str, value: &str) -> Vec<u8> {
    let body = format!("{key}={value}\n");
    let mut length = body.len() + 3;
    loop {
        let next = body.len() + length.to_string().len() + 1;
        if next == length {
            return format!("{length} {body}").into_bytes();
        }
        length = next;
    }
}

fn metadata_count_archive(count: usize) -> Vec<u8> {
    let body = pax_record("a", "b");
    let mut tar = Vec::with_capacity(count * 1024 + 1024);
    for index in 0..count {
        append_raw_header(&mut tar, b"pax", body.len() as u64, &body, b'x');
        let name = format!("package/m{index}");
        append_raw_header(&mut tar, name.as_bytes(), 0, &[], b'0');
    }
    tar.resize(tar.len() + 1024, 0);
    let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
    gzip.write_all(&tar).unwrap();
    gzip.finish().unwrap()
}

fn mutate_tar_gzip(bytes: &[u8], mutate: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    let mut tar = Vec::new();
    GzDecoder::new(bytes).read_to_end(&mut tar).unwrap();
    mutate(&mut tar);
    let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
    gzip.write_all(&tar).unwrap();
    gzip.finish().unwrap()
}

fn descriptor(hash: &str) -> ArtifactDescriptor {
    ArtifactDescriptor {
        artifact_id: "pyright".into(),
        package_name: "pyright".into(),
        version: "1.1.400".into(),
        archive_format: "tar.gz".into(),
        expected_sha256: hash.into(),
        package_root: "package".into(),
        entrypoint: "langserver.index.js".into(),
        runtime: LspArtifactRuntime::Node {
            minimum_version: legion_lsp::LspNodeVersion {
                major: 14,
                minor: 0,
                patch: 0,
            },
        },
    }
}

fn request(d: ArtifactDescriptor, archive: &Path, cache: &Path) -> MaterializeRequest {
    let mut req = MaterializeRequest::local(
        d,
        archive,
        cache,
        7,
        CorrelationId(1),
        CausalityId(Uuid::new_v4()),
    );
    req.trusted = true;
    req
}

#[test]
fn local_import_verifies_before_extracting_and_publishes_cache() {
    let dir = tempdir().unwrap();
    let bytes = archive(&[
        (
            "package/package.json",
            br#"{"name":"pyright","version":"1.1.400"}"#,
        ),
        ("package/langserver.index.js", b"entry"),
    ]);
    let archive_path = dir.path().join("pyright.tgz");
    std::fs::write(&archive_path, &bytes).unwrap();
    let hash = hex::encode(Sha256::digest(&bytes));
    let result = LanguageArtifactMaterializer::materialize(&request(
        descriptor(&hash),
        &archive_path,
        &dir.path().join("cache"),
    ))
    .unwrap();
    assert!(result.cache_root.join(".legion-manifest.json").is_file());
    assert_eq!(result.operation_id, 7);
    assert_eq!(result.correlation_id, CorrelationId(1));
    assert_eq!(
        std::fs::read_to_string(result.cache_root.join("package/langserver.index.js")).unwrap(),
        "entry"
    );
    let reused = LanguageArtifactMaterializer::materialize(&request(
        descriptor(&hash),
        &archive_path,
        &dir.path().join("cache"),
    ))
    .unwrap();
    assert_eq!(reused.cache_root, result.cache_root);
}

#[test]
fn local_import_uses_checked_effective_pax_size_for_preflight_and_extract() {
    let dir = tempdir().unwrap();
    let bytes = raw_pax_size_override_archive(5);
    let path = dir.path().join("pax-size.tgz");
    std::fs::write(&path, &bytes).unwrap();
    let hash = hex::encode(Sha256::digest(&bytes));
    let result = LanguageArtifactMaterializer::materialize(&request(
        descriptor(&hash),
        &path,
        &dir.path().join("cache"),
    ))
    .unwrap();
    assert_eq!(
        std::fs::read(result.cache_root.join("package/data")).unwrap(),
        b"hello"
    );
}

#[test]
fn local_import_rejects_oversize_effective_pax_size_before_extract() {
    let dir = tempdir().unwrap();
    let bytes = raw_pax_size_override_archive(256 * 1024 * 1024 + 1);
    let path = dir.path().join("pax-size-too-large.tgz");
    std::fs::write(&path, &bytes).unwrap();
    let hash = hex::encode(Sha256::digest(&bytes));
    let err = LanguageArtifactMaterializer::materialize(&request(
        descriptor(&hash),
        &path,
        &dir.path().join("cache"),
    ))
    .unwrap_err();
    assert!(matches!(err, MaterializeError::LimitExceeded("file bytes")));
}

#[test]
fn local_import_rejects_global_pax_and_materializing_acl_metadata() {
    for (label, typ, body) in [
        ("global", b'g', pax_record("path", "package/a")),
        ("acl", b'x', pax_record("SCHILY.acl.access", "user::rwx")),
        (
            "xattr",
            b'x',
            pax_record("LIBARCHIVE.xattr.user.test", "value"),
        ),
    ] {
        let dir = tempdir().unwrap();
        let bytes = raw_archive(&[(b"pax", &body, typ)]);
        let path = dir.path().join(format!("{label}.tgz"));
        std::fs::write(&path, &bytes).unwrap();
        let hash = hex::encode(Sha256::digest(&bytes));
        let err = LanguageArtifactMaterializer::materialize(&request(
            descriptor(&hash),
            &path,
            &dir.path().join("cache"),
        ))
        .unwrap_err();
        assert!(
            matches!(err, MaterializeError::InvalidArchive(_)),
            "{label}: {err:?}"
        );
    }
}

#[test]
fn local_import_rejects_consecutive_local_pax_records() {
    let body = pax_record("path", "package/a");
    let bytes = raw_archive(&[(b"pax", &body, b'x'), (b"pax2", &body, b'x')]);
    let dir = tempdir().unwrap();
    let path = dir.path().join("consecutive-pax.tgz");
    std::fs::write(&path, &bytes).unwrap();
    let hash = hex::encode(Sha256::digest(&bytes));
    let err = LanguageArtifactMaterializer::materialize(&request(
        descriptor(&hash),
        &path,
        &dir.path().join("cache"),
    ))
    .unwrap_err();
    assert!(matches!(err, MaterializeError::InvalidArchive(_)));
}

#[test]
fn local_import_accepts_one_gnu_long_name_and_one_local_pax_in_either_order() {
    for (label, gnu_first) in [("gnu-then-pax", true), ("pax-then-gnu", false)] {
        let dir = tempdir().unwrap();
        let bytes = raw_gnu_long_and_local_pax_archive(gnu_first);
        let path = dir.path().join(format!("{label}.tgz"));
        std::fs::write(&path, &bytes).unwrap();
        let hash = hex::encode(Sha256::digest(&bytes));
        let result = LanguageArtifactMaterializer::materialize(&request(
            descriptor(&hash),
            &path,
            &dir.path().join("cache"),
        ))
        .unwrap();
        assert_eq!(
            std::fs::read(result.cache_root.join("package/gnu-long")).unwrap(),
            b"hello",
            "maintained tar parser effective path for {label}"
        );
        assert!(!result.cache_root.join("package/pax-long").exists());
    }
}

#[test]
fn local_import_counts_metadata_records_against_entry_limit() {
    let dir = tempdir().unwrap();
    let bytes = metadata_count_archive(50_001);
    let path = dir.path().join("metadata-count.tgz");
    std::fs::write(&path, &bytes).unwrap();
    let hash = hex::encode(Sha256::digest(&bytes));
    let err = LanguageArtifactMaterializer::materialize(&request(
        descriptor(&hash),
        &path,
        &dir.path().join("cache"),
    ))
    .unwrap_err();
    assert!(matches!(err, MaterializeError::LimitExceeded("entries")));
}

#[test]
fn local_import_hash_mismatch_leaves_no_extracted_files() {
    let dir = tempdir().unwrap();
    let bytes = archive(&[
        (
            "package/package.json",
            br#"{"name":"pyright","version":"1.1.400"}"#,
        ),
        ("package/langserver.index.js", b"entry"),
    ]);
    let archive_path = dir.path().join("bad.tgz");
    std::fs::write(&archive_path, &bytes).unwrap();
    let err = LanguageArtifactMaterializer::materialize(&request(
        descriptor(&"00".repeat(32)),
        &archive_path,
        &dir.path().join("cache"),
    ))
    .unwrap_err();
    assert!(matches!(err, MaterializeError::HashMismatch { .. }));
    assert!(!dir.path().join("cache/sha256").exists());
}

#[test]
fn local_import_reports_hash_mismatch_before_malformed_archive() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("malformed.tgz");
    std::fs::write(&path, b"not a gzip or tar archive").unwrap();
    let err = LanguageArtifactMaterializer::materialize(&request(
        descriptor(&"00".repeat(32)),
        &path,
        &dir.path().join("cache"),
    ))
    .unwrap_err();
    assert!(matches!(err, MaterializeError::HashMismatch { .. }));
    assert!(!dir.path().join("cache/sha256").exists());
}

#[test]
fn local_import_rejects_malformed_numeric_checksum_and_trailing_tar() {
    let base = archive(&[(
        "package/package.json",
        br#"{"name":"pyright","version":"1.1.400"}"#,
    )]);
    let malformed = [
        ("checksum", mutate_tar_gzip(&base, |tar| tar[0] ^= 1)),
        (
            "numeric",
            mutate_tar_gzip(&base, |tar| tar[124..136].fill(b'z')),
        ),
        ("trailing", mutate_tar_gzip(&base, |tar| tar.push(1))),
        (
            "missing-end",
            mutate_tar_gzip(&base, |tar| tar.truncate(tar.len() - 512)),
        ),
    ];
    for (label, bytes) in malformed {
        let dir = tempdir().unwrap();
        let path = dir.path().join(format!("{label}.tgz"));
        std::fs::write(&path, &bytes).unwrap();
        let hash = hex::encode(Sha256::digest(&bytes));
        let err = LanguageArtifactMaterializer::materialize(&request(
            descriptor(&hash),
            &path,
            &dir.path().join("cache"),
        ))
        .unwrap_err();
        assert!(
            matches!(
                err,
                MaterializeError::InvalidArchive(_) | MaterializeError::Io(_)
            ),
            "{label}: {err:?}"
        );
    }
}

#[test]
fn local_import_caps_nul_regular_header_before_body_read() {
    let mut tar = Vec::new();
    append_raw_header(&mut tar, b"package/huge", 256 * 1024 * 1024 + 1, &[], 0);
    tar.resize(tar.len() + 1024, 0);
    let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
    gzip.write_all(&tar).unwrap();
    let bytes = gzip.finish().unwrap();
    let dir = tempdir().unwrap();
    let path = dir.path().join("nul-regular.tgz");
    std::fs::write(&path, &bytes).unwrap();
    let hash = hex::encode(Sha256::digest(&bytes));
    let err = LanguageArtifactMaterializer::materialize(&request(
        descriptor(&hash),
        &path,
        &dir.path().join("cache"),
    ))
    .unwrap_err();
    assert!(matches!(err, MaterializeError::LimitExceeded("file bytes")));
}

#[test]
fn local_import_rejects_traversal_before_creating_destination() {
    let dir = tempdir().unwrap();
    let bytes = raw_unsafe_archive(b"../escape", b"bad");
    let archive_path = dir.path().join("unsafe.tgz");
    std::fs::write(&archive_path, &bytes).unwrap();
    let hash = hex::encode(Sha256::digest(&bytes));
    let err = LanguageArtifactMaterializer::materialize(&request(
        descriptor(&hash),
        &archive_path,
        &dir.path().join("cache"),
    ))
    .unwrap_err();
    assert!(matches!(
        err,
        MaterializeError::UnsafePath(_) | MaterializeError::InvalidArchive(_)
    ));
    assert!(!dir.path().join("escape").exists());
}

#[test]
fn local_import_keeps_tampered_cache_occupied_and_reports_integrity() {
    let dir = tempdir().unwrap();
    let bytes = archive(&[
        (
            "package/package.json",
            br#"{"name":"pyright","version":"1.1.400"}"#,
        ),
        ("package/langserver.index.js", b"entry"),
    ]);
    let archive_path = dir.path().join("tamper.tgz");
    std::fs::write(&archive_path, &bytes).unwrap();
    let hash = hex::encode(Sha256::digest(&bytes));
    let req = request(descriptor(&hash), &archive_path, &dir.path().join("cache"));
    let result = LanguageArtifactMaterializer::materialize(&req).unwrap();
    let entry = result.cache_root.join("package/langserver.index.js");
    std::fs::write(&entry, b"tampered").unwrap();
    let err = LanguageArtifactMaterializer::materialize(&req).unwrap_err();
    assert!(matches!(err, MaterializeError::CacheTampered));
    assert_eq!(std::fs::read(entry).unwrap(), b"tampered");
}

#[test]
fn local_import_rejects_coordinated_manifest_tamper_and_omitted_rows() {
    let dir = tempdir().unwrap();
    let bytes = archive(&[
        (
            "package/package.json",
            br#"{"name":"pyright","version":"1.1.400"}"#,
        ),
        ("package/langserver.index.js", b"entry"),
    ]);
    let archive_path = dir.path().join("manifest-tamper.tgz");
    std::fs::write(&archive_path, &bytes).unwrap();
    let hash = hex::encode(Sha256::digest(&bytes));
    let req = request(descriptor(&hash), &archive_path, &dir.path().join("cache"));
    let result = LanguageArtifactMaterializer::materialize(&req).unwrap();
    let side = result.cache_root.join(".legion-manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&side).unwrap()).unwrap();
    let files = manifest["files"].as_array_mut().unwrap();
    files.retain(|f| f["path"] != "package/langserver.index.js");
    std::fs::write(&side, serde_json::to_vec(&manifest).unwrap()).unwrap();
    std::fs::write(
        result.cache_root.join("package/langserver.index.js"),
        b"coordinated",
    )
    .unwrap();
    let err = LanguageArtifactMaterializer::materialize(&req).unwrap_err();
    assert!(matches!(err, MaterializeError::CacheTampered));
    assert_eq!(
        std::fs::read(result.cache_root.join("package/langserver.index.js")).unwrap(),
        b"coordinated"
    );
}

#[test]
fn local_import_denies_untrusted_context_before_cache_effects() {
    let dir = tempdir().unwrap();
    let archive_path = dir.path().join("missing.tgz");
    let mut req = request(
        descriptor(&"00".repeat(32)),
        &archive_path,
        &dir.path().join("cache"),
    );
    req.trusted = false;
    let err = LanguageArtifactMaterializer::materialize(&req).unwrap_err();
    assert!(matches!(err, MaterializeError::Denied(_)));
    assert!(!dir.path().join("cache").exists());
}

#[test]
fn local_import_rejects_links_devices_duplicates_and_windows_names() {
    for (label, entries) in [
        (
            "link",
            vec![(b"package/link".as_slice(), b"target".as_slice(), b'2')],
        ),
        (
            "device",
            vec![(b"package/device".as_slice(), b"".as_slice(), b'3')],
        ),
        (
            "sparse",
            vec![(b"package/sparse".as_slice(), b"".as_slice(), b'S')],
        ),
        (
            "contiguous",
            vec![(b"package/contiguous".as_slice(), b"".as_slice(), b'7')],
        ),
        (
            "fifo",
            vec![(b"package/fifo".as_slice(), b"".as_slice(), b'6')],
        ),
        (
            "socket",
            vec![(b"package/socket".as_slice(), b"".as_slice(), b's')],
        ),
        (
            "hardlink",
            vec![(b"package/hardlink".as_slice(), b"target".as_slice(), b'1')],
        ),
        (
            "windows",
            vec![(b"package/CON.txt".as_slice(), b"x".as_slice(), b'0')],
        ),
        (
            "duplicate",
            vec![
                (b"package/a".as_slice(), b"x".as_slice(), b'0'),
                (b"package/a".as_slice(), b"y".as_slice(), b'0'),
            ],
        ),
        (
            "ancestor",
            vec![
                (b"package/a".as_slice(), b"x".as_slice(), b'0'),
                (b"package/a/b".as_slice(), b"y".as_slice(), b'0'),
            ],
        ),
        (
            "repeated-separator",
            vec![(b"package//a".as_slice(), b"x".as_slice(), b'0')],
        ),
    ] {
        let dir = tempdir().unwrap();
        let bytes = raw_archive(&entries);
        let archive_path = dir.path().join(format!("{label}.tgz"));
        std::fs::write(&archive_path, &bytes).unwrap();
        let hash = hex::encode(Sha256::digest(&bytes));
        let err = LanguageArtifactMaterializer::materialize(&request(
            descriptor(&hash),
            &archive_path,
            &dir.path().join("cache"),
        ))
        .unwrap_err();
        assert!(
            matches!(
                err,
                MaterializeError::InvalidArchive(_) | MaterializeError::UnsafePath(_)
            ),
            "{label}: {err:?}"
        );
    }
}

#[test]
fn local_import_accepts_bounded_gnu_and_pax_long_names_and_rejects_metadata_budget() {
    let long_name = format!("package/{}", "x".repeat(200));
    let gnu_bytes = archive(&[
        (&long_name, b"long"),
        (
            "package/package.json",
            br#"{"name":"pyright","version":"1.1.400"}"#,
        ),
        ("package/langserver.index.js", b"entry"),
    ]);
    let dir = tempdir().unwrap();
    let gnu_path = dir.path().join("gnu.tgz");
    std::fs::write(&gnu_path, &gnu_bytes).unwrap();
    let gnu_hash = hex::encode(Sha256::digest(&gnu_bytes));
    let gnu_result = LanguageArtifactMaterializer::materialize(&request(
        descriptor(&gnu_hash),
        &gnu_path,
        &dir.path().join("gnu-cache"),
    ))
    .unwrap();
    assert!(gnu_result.cache_root.join(&long_name).is_file());
    let dir = tempdir().unwrap();
    let bytes = pax_archive(&long_name, b"long");
    let archive_path = dir.path().join("pax.tgz");
    std::fs::write(&archive_path, &bytes).unwrap();
    let hash = hex::encode(Sha256::digest(&bytes));
    let result = LanguageArtifactMaterializer::materialize(&request(
        descriptor(&hash),
        &archive_path,
        &dir.path().join("cache"),
    ))
    .unwrap();
    assert!(result.cache_root.join(&long_name).is_file());

    let huge = vec![b'a'; 65 * 1024];
    let bytes = raw_archive(&[
        (b"./long", &huge, b'L'),
        (
            b"package/package.json",
            br#"{"name":"pyright","version":"1.1.400"}"#,
            b'0',
        ),
    ]);
    let path = dir.path().join("metadata-budget.tgz");
    std::fs::write(&path, &bytes).unwrap();
    let hash = hex::encode(Sha256::digest(&bytes));
    let err = LanguageArtifactMaterializer::materialize(&request(
        descriptor(&hash),
        &path,
        &dir.path().join("cache2"),
    ))
    .unwrap_err();
    assert!(matches!(
        err,
        MaterializeError::LimitExceeded("tar metadata")
    ));
}

#[test]
fn local_import_async_terminal_delivery_and_deadline_cleanup_are_bounded() {
    let dir = tempdir().unwrap();
    let bytes = archive(&[
        (
            "package/package.json",
            br#"{"name":"pyright","version":"1.1.400"}"#,
        ),
        ("package/langserver.index.js", b"entry"),
    ]);
    let archive_path = dir.path().join("deadline.tgz");
    std::fs::write(&archive_path, &bytes).unwrap();
    let hash = hex::encode(Sha256::digest(&bytes));
    let mut req = request(
        descriptor(&hash),
        &archive_path,
        &dir.path().join("cache-async"),
    );
    req.deadline = std::time::Duration::from_nanos(1);
    let handle = LanguageArtifactMaterializer::start(req);
    let mut terminal = false;
    for _ in 0..16 {
        if let Ok(event) = handle.recv()
            && matches!(event, legion_app::language::MaterializeEvent::Failed(_))
        {
            terminal = true;
            break;
        }
    }
    assert!(terminal);
    assert!(!dir.path().join("cache-async/sha256").exists());
    assert!(
        std::fs::read_dir(dir.path().join("cache-async"))
            .map(|entries| entries
                .flatten()
                .all(|e| !e.file_name().to_string_lossy().starts_with(".staging-")))
            .unwrap_or(true)
    );
}

#[test]
fn local_import_cancellation_cleans_private_staging() {
    let dir = tempdir().unwrap();
    // Keep the copy phase in flight so an immediate cancel cannot lose the
    // race against a completed Ready on a fast runner.
    let body = vec![b'x'; 2 * 1024 * 1024];
    let bytes = archive(&[
        (
            "package/package.json",
            br#"{"name":"pyright","version":"1.1.400"}"#,
        ),
        ("package/langserver.index.js", &body),
    ]);
    let archive_path = dir.path().join("cancel.tgz");
    std::fs::write(&archive_path, &bytes).unwrap();
    let hash = hex::encode(Sha256::digest(&bytes));
    let req = request(
        descriptor(&hash),
        &archive_path,
        &dir.path().join("cache-cancel"),
    );
    let handle = LanguageArtifactMaterializer::start(req);
    let mut terminal = false;
    for _ in 0..64 {
        let event = handle.recv().expect("worker delivers until a terminal");
        if matches!(event, legion_app::language::MaterializeEvent::Progress(_)) {
            handle.cancel();
        }
        match event {
            legion_app::language::MaterializeEvent::Failed(MaterializeError::Cancelled) => {
                terminal = true;
                break;
            }
            legion_app::language::MaterializeEvent::Ready(_)
            | legion_app::language::MaterializeEvent::Failed(_) => {
                panic!("expected Cancelled terminal, got {event:?}");
            }
            legion_app::language::MaterializeEvent::Progress(_) => {}
        }
    }
    assert!(terminal);
    assert!(!dir.path().join("cache-cancel/sha256").exists());
    assert!(
        std::fs::read_dir(dir.path().join("cache-cancel"))
            .map(|entries| entries
                .flatten()
                .all(|e| !e.file_name().to_string_lossy().starts_with(".staging-")))
            .unwrap_or(true)
    );
}

#[test]
fn local_import_cancellation_during_copy_delivers_one_terminal_and_cleans_staging() {
    let dir = tempdir().unwrap();
    let body = vec![b'x'; 2 * 1024 * 1024];
    let bytes = archive(&[
        (
            "package/package.json",
            br#"{"name":"pyright","version":"1.1.400"}"#,
        ),
        ("package/langserver.index.js", &body),
    ]);
    let archive_path = dir.path().join("copy-cancel.tgz");
    std::fs::write(&archive_path, &bytes).unwrap();
    let hash = hex::encode(Sha256::digest(&bytes));
    let handle = LanguageArtifactMaterializer::start(request(
        descriptor(&hash),
        &archive_path,
        &dir.path().join("copy-cancel-cache"),
    ));
    let mut terminal_count = 0;
    let mut cancelled = false;
    for _ in 0..32 {
        let event = handle.recv().unwrap();
        if matches!(
            event,
            legion_app::language::MaterializeEvent::Progress(
                legion_app::language::MaterializeProgress::Copying { .. }
            )
        ) {
            handle.cancel();
            cancelled = true;
        }
        if matches!(
            event,
            legion_app::language::MaterializeEvent::Ready(_)
                | legion_app::language::MaterializeEvent::Failed(_)
        ) {
            terminal_count += 1;
            if cancelled {
                break;
            }
        }
    }
    assert!(cancelled);
    assert_eq!(terminal_count, 1);
    assert!(!dir.path().join("copy-cancel-cache/sha256").exists());
}

#[test]
#[ignore = "explicit retained local Pyright fixture validation"]
fn retained_pyright_fixture_materializes_with_catalog_identity() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.superpowers/sdd/2026-09-04-full-product-completion/pyright-1.1.400.tgz");
    assert!(
        fixture.is_file(),
        "retained fixture is unavailable: {fixture:?}"
    );
    let dir = tempdir().unwrap();
    let req = request(
        descriptor("2ccba7af9c8b14bb81c8fa9bb558d8b5181b586ec4dfc448b78eb4209e7a429a"),
        &fixture,
        &dir.path().join("cache"),
    );
    let result = LanguageArtifactMaterializer::materialize(&req).unwrap();
    assert_eq!(result.entries, 4626);
    assert_eq!(result.uncompressed_bytes, 16_302_649);
    assert!(
        result
            .cache_root
            .join("package/langserver.index.js")
            .is_file()
    );
}
