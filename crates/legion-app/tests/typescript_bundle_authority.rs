use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use legion_app::language::{
    LanguageStartupAuthority, LanguageStartupContext, TypeScriptBundleDescriptor,
};
use legion_protocol::{
    CapabilityBrokerPort, CapabilityDecision, CapabilityResponse, CausalityId, CorrelationId,
    LanguageId, LanguageServerId, PrincipalId, WorkspaceId, WorkspaceRootId, WorkspaceTrustState,
};
use sha2::Digest;

struct DecisionBroker {
    requests: Arc<AtomicUsize>,
}

impl CapabilityBrokerPort for DecisionBroker {
    fn handle(
        &self,
        request: legion_protocol::CapabilityRequest,
    ) -> legion_protocol::ProtocolResult<CapabilityResponse> {
        self.requests.fetch_add(1, Ordering::Relaxed);
        let legion_protocol::CapabilityRequest::Request { capability_id, .. } = request else {
            panic!("TypeScript authority must issue a request envelope")
        };
        Ok(CapabilityResponse::Decision(CapabilityDecision {
            decision_id: legion_protocol::CapabilityDecisionId(41),
            granted: true,
            capability: capability_id,
            reason: None,
        }))
    }
}

fn context() -> LanguageStartupContext {
    LanguageStartupContext {
        workspace_id: WorkspaceId(77),
        root_id: WorkspaceRootId(78),
        workspace_root: std::env::current_dir().expect("workspace root"),
        principal_id: PrincipalId("typescript-authority-test".into()),
        trust: WorkspaceTrustState::Trusted,
        correlation_id: CorrelationId(9),
        causality_id: CausalityId(uuid::Uuid::from_u128(9)),
    }
}

fn root_uri() -> String {
    let root = std::fs::canonicalize(std::env::current_dir().unwrap()).unwrap();
    let mut path = root.to_str().unwrap().replace('\\', "/");
    if let Some(rest) = path.strip_prefix("//?/") {
        path = rest.to_string();
    }
    format!("file:///{}", path.trim_start_matches('/'))
}

fn synthetic_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    {
        let mut tar = tar::Builder::new(&mut gzip);
        for (name, body) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, *name, *body)
                .expect("synthetic archive entry");
        }
        tar.finish().expect("synthetic archive finish");
    }
    gzip.finish().expect("synthetic gzip finish")
}

fn synthetic_bundle(dir: &Path) -> (TypeScriptBundleDescriptor, PathBuf, PathBuf) {
    let server_bytes = synthetic_archive(&[
        (
            "package/package.json",
            br#"{"name":"typescript-language-server","version":"6.0.0"}"#,
        ),
        ("package/lib/cli.mjs", b"server"),
    ]);
    let compiler_bytes = synthetic_archive(&[
        (
            "package/package.json",
            br#"{"name":"typescript","version":"6.0.3"}"#,
        ),
        ("package/lib/tsserver.js", b"compiler"),
    ]);
    let server_path = dir.join("server.tgz");
    let compiler_path = dir.join("compiler.tgz");
    std::fs::write(&server_path, &server_bytes).expect("synthetic server archive");
    std::fs::write(&compiler_path, &compiler_bytes).expect("synthetic compiler archive");
    let mut bundle = TypeScriptBundleDescriptor::pinned();
    bundle.server.expected_sha256 = hex::encode(sha2::Sha256::digest(&server_bytes));
    bundle.compiler.expected_sha256 = hex::encode(sha2::Sha256::digest(&compiler_bytes));
    (bundle, server_path, compiler_path)
}

fn fake_node(dir: &Path) -> PathBuf {
    let path = dir.join(if cfg!(windows) { "node.exe" } else { "node" });
    std::fs::write(&path, b"fixture node").expect("fake Node fixture");
    std::fs::canonicalize(path).expect("canonical fake Node fixture")
}

fn invoke(
    context: &LanguageStartupContext,
    bundle: &TypeScriptBundleDescriptor,
    server_archive: &Path,
    compiler_archive: &Path,
    node: &Path,
    cache: &Path,
) -> (
    Result<
        legion_app::language::LanguageServerStartConfig,
        legion_app::language::LanguageSessionError,
    >,
    Arc<AtomicUsize>,
) {
    let requests = Arc::new(AtomicUsize::new(0));
    let result = LanguageStartupAuthority::new(Arc::new(DecisionBroker {
        requests: Arc::clone(&requests),
    }))
    .prepare_typescript_bundle(
        context,
        bundle,
        server_archive,
        compiler_archive,
        node,
        cache,
        Arc::new(AtomicBool::new(false)),
        root_uri(),
        LanguageId("typescript".into()),
        LanguageServerId(102),
    );
    (result, requests)
}

#[test]
fn typescript_bundle_rejects_non_typescript_adapter_before_materialization() {
    let dir = tempfile::tempdir().expect("authority test directory");
    let mut context = context();
    context.workspace_root = dir.path().to_path_buf();
    let requests = Arc::new(AtomicUsize::new(0));
    let result = LanguageStartupAuthority::new(Arc::new(DecisionBroker {
        requests: Arc::clone(&requests),
    }))
    .prepare_typescript_bundle(
        &context,
        &TypeScriptBundleDescriptor::pinned(),
        Path::new("missing-server.tgz"),
        Path::new("missing-compiler.tgz"),
        Path::new("missing-node"),
        &dir.path().join("cache"),
        Arc::new(AtomicBool::new(false)),
        format!(
            "file:///{}",
            dir.path().to_string_lossy().replace('\\', "/")
        ),
        LanguageId("python".into()),
        LanguageServerId(104),
    );
    let error = result
        .err()
        .expect("non-TypeScript language must be rejected");
    assert!(format!("{error:?}").contains("TypeScript-family"));
    assert_eq!(requests.load(Ordering::Relaxed), 0);
    assert!(!dir.path().join("cache").exists());
}

#[test]
fn typescript_bundle_rejects_untrusted_and_zero_identity_before_materialization() {
    let dir = tempfile::tempdir().expect("authority test directory");
    let node = fake_node(dir.path());
    let (bundle, server, compiler) = synthetic_bundle(dir.path());
    for invalid in [
        {
            let mut value = context();
            value.trust = WorkspaceTrustState::Untrusted;
            value
        },
        {
            let mut value = context();
            value.correlation_id = CorrelationId(0);
            value
        },
        {
            let mut value = context();
            value.causality_id = CausalityId(uuid::Uuid::nil());
            value
        },
    ] {
        let cache = dir
            .path()
            .join(format!("cache-{}", invalid.correlation_id.0));
        let (result, requests) = invoke(&invalid, &bundle, &server, &compiler, &node, &cache);
        let error = result.err().expect("invalid context must be rejected");
        let debug = format!("{error:?}");
        assert!(debug.contains("trusted workspace") || debug.contains("event identity"));
        assert_eq!(requests.load(Ordering::Relaxed), 0);
        assert!(!cache.exists());
    }
}

#[test]
fn typescript_bundle_pre_cancelled_request_does_not_materialize_or_probe_node() {
    let dir = tempfile::tempdir().expect("authority test directory");
    let node = fake_node(dir.path());
    let cache = dir.path().join("cache");
    let cancellation = Arc::new(AtomicBool::new(true));
    let (bundle, server, compiler) = synthetic_bundle(dir.path());
    let requests = Arc::new(AtomicUsize::new(0));
    let result = LanguageStartupAuthority::new(Arc::new(DecisionBroker {
        requests: Arc::clone(&requests),
    }))
    .prepare_typescript_bundle(
        &context(),
        &bundle,
        &server,
        &compiler,
        &node,
        &cache,
        cancellation,
        root_uri(),
        LanguageId("typescript".into()),
        LanguageServerId(102),
    );
    let error = result
        .err()
        .expect("pre-cancelled request must be rejected");
    assert!(error.to_string().contains("Cancelled"));
    assert_eq!(requests.load(Ordering::Relaxed), 0);
    assert!(!cache.exists());
}

#[test]
fn typescript_bundle_rejects_wrong_server_or_compiler_archive_hash() {
    let dir = tempfile::tempdir().expect("authority test directory");
    let node = fake_node(dir.path());
    let (bundle, server, compiler) = synthetic_bundle(dir.path());

    let mut wrong_server = bundle.clone();
    wrong_server.server.expected_sha256 = "0".repeat(64);
    let (result, requests) = invoke(
        &context(),
        &wrong_server,
        &server,
        &compiler,
        &node,
        &dir.path().join("server-cache"),
    );
    let error = result.err().expect("wrong server digest must be rejected");
    assert!(error.to_string().contains("HashMismatch"));
    assert_eq!(requests.load(Ordering::Relaxed), 0);

    let mut wrong_compiler = bundle.clone();
    wrong_compiler.compiler.expected_sha256 = "f".repeat(64);
    let (result, requests) = invoke(
        &context(),
        &wrong_compiler,
        &server,
        &compiler,
        &node,
        &dir.path().join("compiler-cache"),
    );
    let error = result
        .err()
        .expect("wrong compiler digest must be rejected");
    assert!(error.to_string().contains("HashMismatch"));
    assert_eq!(requests.load(Ordering::Relaxed), 0);
    assert!(
        dir.path()
            .join("compiler-cache/sha256")
            .join(&bundle.server.expected_sha256)
            .is_dir(),
        "valid server archive must be published before compiler digest rejection"
    );
    assert!(
        !dir.path()
            .join("compiler-cache/sha256")
            .join("f".repeat(64))
            .exists()
    );
}
