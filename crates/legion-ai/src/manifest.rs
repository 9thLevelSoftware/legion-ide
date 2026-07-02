use legion_protocol::{ContextManifestAssembly, ContextManifestRecord};

/// Assemble a metadata-only context manifest record from a structured DTO.
///
/// This helper keeps the AI crate on the structured-DTO path and avoids any
/// freeform prompt serialization for the manifest payload itself.
pub fn assemble_context_manifest(assembly: ContextManifestAssembly) -> ContextManifestRecord {
    assembly.into_record()
}
