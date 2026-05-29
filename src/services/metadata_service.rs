use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::SystemTime;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

use crate::domain::save::SaveMetadata;

pub fn collect_metadata(path: &Path) -> Result<SaveMetadata> {
    let file_metadata =
        fs::metadata(path).with_context(|| format!("failed to read metadata {}", path.display()))?;
    let sha256 = compute_sha256(path)?;

    Ok(SaveMetadata {
        modified_at: system_time_to_utc(file_metadata.modified().ok()),
        created_at: system_time_to_utc(file_metadata.created().ok()),
        byte_size: file_metadata.len(),
        sha256: Some(sha256),
    })
}

pub fn verify_copy_integrity(source_path: &Path, destination_path: &Path) -> Result<()> {
    let source = collect_metadata(source_path)?;
    let destination = collect_metadata(destination_path)?;
    verify_metadata_pair(&source, &destination)
}

pub fn verify_metadata_pair(source: &SaveMetadata, destination: &SaveMetadata) -> Result<()> {
    if source.byte_size != destination.byte_size {
        anyhow::bail!(
            "integrity verification failed: byte size mismatch (src={}, dst={})",
            source.byte_size,
            destination.byte_size
        );
    }

    match (source.sha256.as_deref(), destination.sha256.as_deref()) {
        (Some(a), Some(b)) if a == b => Ok(()),
        (Some(_), Some(_)) => anyhow::bail!("integrity verification failed: hash mismatch"),
        _ => anyhow::bail!("integrity verification failed: missing hash metadata"),
    }
}

fn compute_sha256(path: &Path) -> Result<String> {
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open file {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];

    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("failed to read file {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn system_time_to_utc(time: Option<SystemTime>) -> Option<DateTime<Utc>> {
    time.map(DateTime::<Utc>::from)
}
