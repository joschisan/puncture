use std::{fs, path::Path};

use rand::Rng;

/// Read an existing secret key from file, or generate and save a new one
pub fn read_or_generate(data_dir: &Path) -> iroh::SecretKey {
    let file_path = data_dir.join("puncture_secret.key");

    if file_path.exists() {
        let bytes = fs::read(file_path)
            .expect("Failed to read secret file")
            .try_into()
            .unwrap();

        return iroh::SecretKey::from_bytes(&bytes);
    }

    let secret: [u8; 32] = rand::rng().random();

    fs::write(&file_path, secret).expect("Failed to write secret file");

    iroh::SecretKey::from_bytes(&secret)
}

/// Check if the this is the first startup of the daemon
pub fn exists(data_dir: &Path) -> bool {
    data_dir.join("puncture_secret.key").exists()
}
