//! Data structures for describing chunks of data
//!
//! Contains structs representing both encrypted and unencrypted data

use super::{Compression, Encryption, Key, HMAC};
use serde::{Deserialize, Serialize};
use std::cmp;

/// Key for an object in a repository
#[derive(PartialEq, Eq, Copy, Clone, Serialize, Deserialize, Hash, Debug)]
pub struct ChunkID {
    /// Keys are a bytestring of length 32
    ///
    /// This lines up well with SHA256 and other 256 bit hashes.
    /// Longer hashes will be truncated and shorter ones (not reccomended) will be padded.
    id: [u8; 32],
}

impl ChunkID {
    /// Will create a new key from a slice.
    ///
    /// Keys longer than 32 bytes will be truncated.
    /// Keys shorter than 32 bytes will be padded at the end with zeros.
    pub fn new(input_id: &[u8]) -> ChunkID {
        let mut id: [u8; 32] = [0; 32];
        id[..cmp::min(32, input_id.len())]
            .clone_from_slice(&input_id[..cmp::min(32, input_id.len())]);
        ChunkID { id }
    }

    /// Returns an immutable refrence to the key in bytestring form
    pub fn get_id(&self) -> &[u8] {
        &self.id
    }

    /// Verifies equaliy of this key with the first 32 bytes of a slice
    pub fn verfiy(&self, slice: &[u8]) -> bool {
        if slice.len() < self.id.len() {
            false
        } else {
            let mut equal = true;
            for (i, val) in self.id.iter().enumerate() {
                if *val != slice[i] {
                    equal = false;
                }
            }
            equal
        }
    }

    /// Returns the special all-zero key used for the manifest
    pub fn manifest_id() -> ChunkID {
        ChunkID { id: [0_u8; 32] }
    }
}

/// Chunk Settings
///
/// Encapsulates the Encryption, Compression, and HMAC tags for a chunk
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChunkSettings {
    pub compression: Compression,
    pub encryption: Encryption,
    pub hmac: HMAC,
}

/// A raw block of data and its associated ChunkID
///
/// This data is not encrypted, compressed, or otherwise tampered with, and can not be directly
/// inserted into the repo.
pub struct UnpackedChunk {
    data: Vec<u8>,
    id: ChunkID,
}

impl UnpackedChunk {
    /// Creates a new unpacked chunk
    ///
    /// HMAC algorthim used for chunkid is specified by chunksettings
    ///
    /// Key used for ChunkID generation is determined by key
    pub fn new(data: Vec<u8>, settings: &ChunkSettings, key: &Key) -> UnpackedChunk {
        let id = ChunkID::new(&settings.hmac.id(data.as_slice(), key));
        UnpackedChunk { data, id }
    }

    /// Returns the chunkid
    pub fn id(&self) -> ChunkID {
        self.id
    }

    /// Returns a refrence to the data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Consumes self and returns a real chunk
    pub fn pack(self, settings: &ChunkSettings, key: &Key) -> Chunk {
        Chunk::pack_with_id(
            self.data,
            settings.compression,
            settings.encryption,
            settings.hmac,
            key,
            self.id,
        )
    }

    /// Returns the data consuming self
    pub fn consuming_data(self) -> Vec<u8> {
        self.data
    }
}

/// Data chunk
///
/// Encrypted, compressed object, to be stored in the repository
#[derive(Serialize, Deserialize)]
pub struct Chunk {
    /// The data of the chunk, stored as a vec of raw bytes
    #[serde(with = "serde_bytes")]
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
    #[serde(with = "serde_bytes")]
    mac: Vec<u8>,
    /// Chunk ID, generated from the HMAC
    id: ChunkID,
}

impl Chunk {
    #[cfg_attr(feature = "profile", flame)]
    /// Will Pack the data into a chunk with the given compression and encryption
    pub fn pack(
        data: Vec<u8>,
        compression: Compression,
        encryption: Encryption,
        hmac: HMAC,
        key: &Key,
    ) -> Chunk {
        let id_mac = hmac.id(&data, key);
        let compressed_data = compression.compress(data);
        let data = encryption.encrypt(&compressed_data, key);
        let id = ChunkID::new(&id_mac);
        let mac = hmac.mac(&data, key);
        Chunk {
            data,
            compression,
            encryption,
            hmac,
            mac,
            id,
        }
    }

    #[cfg_attr(feature = "profile", flame)]
    /// Will pack a chunk, but manually setting the id instead of hashing
    ///
    /// This function should be used carefully, as it has potentiall to do major damage to the repository
    pub fn pack_with_id(
        data: Vec<u8>,
        compression: Compression,
        encryption: Encryption,
        hmac: HMAC,
        key: &Key,
        id: ChunkID,
    ) -> Chunk {
        let compressed_data = compression.compress(data);
        let data = encryption.encrypt(&compressed_data, key);
        let mac = hmac.mac(&data, key);
        Chunk {
            data,
            compression,
            encryption,
            hmac,
            mac,
            id,
        }
    }

    #[cfg_attr(feature = "profile", flame)]
    /// Decrypts and decompresses the data in the chunk
    ///
    /// Will return none if either the decompression or the decryption fail
    ///
    /// Will also return none if the HMAC verification fails
    pub fn unpack(&self, key: &Key) -> Option<Vec<u8>> {
        if self.hmac.verify_hmac(&self.mac, &self.data, key) {
            let decrypted_data = self.encryption.decrypt(&self.data, key)?;
            let decompressed_data = self.compression.decompress(decrypted_data)?;

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
        id: ChunkID,
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
    pub fn get_id(&self) -> ChunkID {
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
    use tempfile::{tempdir, TempDir};

    fn chunk_with_settings(compression: Compression, encryption: Encryption, hmac: HMAC) {
        let data_string =
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.";

        let data_bytes = data_string.as_bytes().to_vec();
        println!("Data: \n:{:X?}", data_bytes);

        let key = Key::random(32);
        let packed = Chunk::pack(data_bytes, compression, encryption, hmac, &key);

        let output_bytes = packed.unpack(&key);

        assert_eq!(Some(data_string.as_bytes().to_vec()), output_bytes);
    }

    #[test]
    fn chunk_aes256cbc_zstd6_sha256() {
        let compression = Compression::ZStd { level: 6 };
        let encryption = Encryption::new_aes256cbc();
        let hmac = HMAC::SHA256;
        chunk_with_settings(compression, encryption, hmac);
    }

    #[test]
    fn chunk_aes256cbc_zstd6_blake2b() {
        let compression = Compression::ZStd { level: 6 };
        let encryption = Encryption::new_aes256cbc();
        let hmac = HMAC::Blake2b;
        chunk_with_settings(compression, encryption, hmac);
    }

    #[test]
    fn chunk_aes256ctr_zstd6_blake2b() {
        let compression = Compression::ZStd { level: 6 };
        let encryption = Encryption::new_aes256ctr();
        let hmac = HMAC::Blake2b;
        chunk_with_settings(compression, encryption, hmac);
    }

    #[test]
    fn detect_bad_data() {
        let data_string = "I am but a humble test string";
        let data_bytes = data_string.as_bytes().to_vec();
        let compression = Compression::NoCompression;
        let encryption = Encryption::NoEncryption;
        let hmac = HMAC::SHA256;

        let key = Key::random(32);

        let mut packed = Chunk::pack(data_bytes, compression, encryption, hmac, &key);
        packed.break_data(5);

        let result = packed.unpack(&key);

        assert_eq!(result, None);
    }
}
