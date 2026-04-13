use base64::{Engine as _, engine::general_purpose};
use rand::{RngCore, SeedableRng};
use sha2::{Digest, Sha256};

pub fn hash_sha256(value: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hasher.finalize().to_vec()
}

pub fn random_bytes_as_text(bytes: usize) -> String {
    let mut rng = rand::rngs::OsRng;
    let mut buf = vec![0u8; bytes];
    rng.fill_bytes(&mut buf);
    
    // Convert to base64
    let encoded = general_purpose::STANDARD.encode(buf);
    
    // In TS: return randomBytes(bytes).toString('base64').replaceAll(/\W/g, '');
    // \W matches any non-word character (equivalent to [^a-zA-Z0-9_]).
    encoded
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

pub fn compare_bcrypt(password: &str, hash: &str) -> bool {
    // In bcrypt crate, verify returns Result<bool, BcryptError>
    // We just return true if Ok(true).
    bcrypt::verify(password, hash).unwrap_or(false)
}
