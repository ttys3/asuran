use std::cmp;

use crate::repository::backend::*;
use crate::repository::compression::*;
use crate::repository::encryption::*;
use crate::repository::hmac::*;

pub mod backend;
pub mod compression;
pub mod encryption;
pub mod hmac;

pub struct Repository {
    backend: Box<dyn Backend>,
}

/// Key for an object in a repository
#[derive(PartialEq, Eq, Copy, Clone)]
pub struct Key {
    /// Keys are a bytestring of length 32
    ///
    /// This lines up well with SHA256 and other 256 bit hashes.
    /// Longer hashes will be truncated and shorter ones (not reccomended) will be padded.
    key: [u8; 32],
}

impl Key {
    /// Will create a new key from a slice.
    ///
    /// Keys longer than 32 bytes will be truncated.
    /// Keys shorter than 32 bytes will be padded at the end with zeros.
    pub fn new(input_key: &[u8]) -> Key {
        let mut key: [u8; 32] = [0; 32];
        key[..cmp::min(32, input_key.len())]
            .clone_from_slice(&input_key[..cmp::min(32, input_key.len())]);
        Key { key }
    }

    /// Returns an immutable refrence to the key in bytestring form
    pub fn get_key(&self) -> &[u8] {
        &self.key
    }
}

/// Data chunk
///
/// Encrypted, compressed object, to be stored in the repository
pub struct Chunk {
    /// The data of the chunk, stored as a vec of raw bytes
    data: Vec<u8>,
    /// Compression algorithim used
    compression: Compression,
    /// Encryption Algorithim used, also stores IV
    encryption: Encryption,
    /// HMAC algorithim used
    ///
    /// HAMC key is also the same as the repo encryption key
    hmac: HMAC,
    /// Actual MAC value of this chunk
    mac: Vec<u8>,
    /// Chunk ID, generated from the HMAC
    id: Key,
}

impl Chunk {
    /// Will Pack the data into a chunk with the given compression and encryption
    pub fn pack(
        data: &[u8],
        compression: Compression,
        encryption: Encryption,
        hmac: HMAC,
        key: &[u8],
    ) -> Chunk {
        let mac = hmac.mac(&data, key);
        let compressed_data = compression.compress(data);
        let data = encryption.encrypt(&compressed_data, key);
        let id = Key::new(&mac);
        Chunk {
            data,
            compression,
            encryption,
            hmac,
            mac,
            id,
        }
    }

    /// Decrypts and decompresses the data in the chunk
    ///
    /// Will return none if either the decompression or the decryption fail
    ///
    /// Will also return none if the HMAC verification fails
    pub fn unpack(&self, key: &[u8]) -> Option<Vec<u8>> {
        let decrypted_data = self.encryption.decrypt(&self.data, key)?;
        let decompressed_data = self.compression.decompress(&decrypted_data)?;

        if self.hmac.verify(&self.mac, &decompressed_data, key) {
            Some(decompressed_data)
        } else {
            None
        }
    }

    /// Creates a chunk from a raw bytestring with the given compressor
    /// and encryption algorithim
    pub fn from_bytes(
        data: &[u8],
        compression: Compression,
        encryption: Encryption,
        hmac: HMAC,
        mac: &[u8],
        id: Key,
    ) -> Chunk {
        Chunk {
            data: data.to_vec(),
            compression,
            encryption,
            hmac,
            mac: mac.to_vec(),
            id,
        }
    }

    /// Returns the length of the data bytes
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Determine if this chunk is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns a reference to the raw bytes of this chunk
    pub fn get_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Gets the key for this block
    pub fn get_id(&self) -> Key {
        self.id
    }

    #[cfg(test)]
    /// Testing only function used to corrupt the data
    pub fn break_data(&mut self, index: usize) {
        let val = self.data[index];
        if val == 0 {
            self.data[index] = 1;
        } else {
            self.data[index] = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::*;
    use std::str;

    #[test]
    fn chunk_aes256cbc_zstd6() {
        let data_string =
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.";

        let data_bytes = data_string.as_bytes();
        println!("Data: \n:{:X?}", data_bytes);
        let compression = Compression::ZStd { level: 6 };
        let encryption = Encryption::new_aes256cbc();
        let hmac = HMAC::SHA256;

        let mut key: [u8; 32] = [0; 32];
        thread_rng().fill_bytes(&mut key);

        let packed = Chunk::pack(&data_bytes, compression, encryption, hmac, &key);

        let output_bytes = packed.unpack(&key);

        assert_eq!(Some(data_string.as_bytes().to_vec()), output_bytes);
    }

    #[test]
    fn detect_bad_data() {
        let data_string = "I am but a humble test string";
        let data_bytes = data_string.as_bytes();
        let compression = Compression::NoCompression;
        let encryption = Encryption::NoEncryption;
        let hmac = HMAC::SHA256;

        let mut key: [u8; 32] = [0; 32];
        thread_rng().fill_bytes(&mut key);

        let mut packed = Chunk::pack(&data_bytes, compression, encryption, hmac, &key);
        packed.break_data(5);

        let result = packed.unpack(&key);

        assert_eq!(result, None);
    }

}
