//! Emit canonical Gate 1B1 corpus JSON (numerical records only).

use relativity_integrate::canonical_corpus_json;
use sha2::{Digest, Sha256};

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let json = canonical_corpus_json()?;
    // Print JSON only — digest is of these bytes.
    print!("{json}");
    let _ = Sha256::digest(json.as_bytes());
    Ok(())
}

pub fn digest_of(json: &str) -> String {
    Sha256::digest(json.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
