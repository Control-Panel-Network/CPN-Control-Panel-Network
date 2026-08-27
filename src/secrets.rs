use base64::{Engine, engine::general_purpose::STANDARD};
use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit,
    aead::{Aead, Payload},
};
use rand::Rng;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
};

const AAD: &[u8] = b"cpn-secret-store-v1";

#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u8,
    nonce: String,
    ciphertext: String,
}

fn private_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
    }
    let temporary = path.with_extension("tmp");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| error.to_string())
}

pub fn load_or_create_key(path: &Path) -> Result<[u8; 32], String> {
    if path.exists() {
        let bytes = fs::read(path).map_err(|error| error.to_string())?;
        return bytes
            .try_into()
            .map_err(|_| "La clave local de CPN tiene un tamaño inválido".to_string());
    }
    let mut key = [0_u8; 32];
    rand::rng().fill(&mut key);
    private_write(path, &key)?;
    Ok(key)
}

pub fn seal<T: Serialize>(path: &Path, key: &[u8; 32], value: &T) -> Result<(), String> {
    let plaintext = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    let mut nonce = [0_u8; 12];
    rand::rng().fill(&mut nonce);
    let cipher = ChaCha20Poly1305::new(key.into());
    let ciphertext = cipher
        .encrypt(
            (&nonce).into(),
            Payload {
                msg: &plaintext,
                aad: AAD,
            },
        )
        .map_err(|_| "No se pudo cifrar el secreto".to_string())?;
    let envelope = Envelope {
        version: 1,
        nonce: STANDARD.encode(nonce),
        ciphertext: STANDARD.encode(ciphertext),
    };
    private_write(
        path,
        &serde_json::to_vec(&envelope).map_err(|error| error.to_string())?,
    )
}

pub fn open<T: DeserializeOwned>(path: &Path, key: &[u8; 32]) -> Result<T, String> {
    let envelope: Envelope = serde_json::from_slice(
        &fs::read(path).map_err(|error| format!("No se pudo leer el secreto: {error}"))?,
    )
    .map_err(|error| error.to_string())?;
    if envelope.version != 1 {
        return Err("La versión del secreto cifrado no es compatible".into());
    }
    let nonce = STANDARD
        .decode(envelope.nonce)
        .map_err(|error| error.to_string())?;
    let nonce: [u8; 12] = nonce
        .try_into()
        .map_err(|_| "El nonce cifrado es inválido".to_string())?;
    let ciphertext = STANDARD
        .decode(envelope.ciphertext)
        .map_err(|error| error.to_string())?;
    let plaintext = ChaCha20Poly1305::new(key.into())
        .decrypt(
            (&nonce).into(),
            Payload {
                msg: &ciphertext,
                aad: AAD,
            },
        )
        .map_err(|_| "No se pudo descifrar el secreto".to_string())?;
    serde_json::from_slice(&plaintext).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{load_or_create_key, open, seal};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Secret {
        token: String,
    }

    #[test]
    fn encrypted_store_round_trips_without_plaintext() {
        let directory = tempfile::tempdir().unwrap();
        let key_path = directory.path().join("key");
        let secret_path = directory.path().join("cloudflare.enc");
        let key = load_or_create_key(&key_path).unwrap();
        let expected = Secret {
            token: "cf-sensitive-token".into(),
        };
        seal(&secret_path, &key, &expected).unwrap();

        assert!(
            !String::from_utf8_lossy(&std::fs::read(&secret_path).unwrap())
                .contains("cf-sensitive-token")
        );
        assert_eq!(open::<Secret>(&secret_path, &key).unwrap(), expected);
        assert_eq!(
            std::fs::metadata(&secret_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    use std::os::unix::fs::PermissionsExt;
}
