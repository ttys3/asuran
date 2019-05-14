use crate::chunker::{Chunker, Slice};
use crate::repository::{Key, Repository};
use chrono::prelude::*;
use rmp_serde::{Deserializer, Serializer};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};

#[cfg(feature = "profile")]
use flame::*;
#[cfg(feature = "profile")]
use flamer::flame;

/// An archive in a repository
#[derive(Serialize, Deserialize, Clone)]
pub struct StoredArchive {
    name: String,
    id: Key,
    timestamp: DateTime<Utc>,
}

/// Location of a chunk in a file
#[derive(Serialize, Deserialize, Copy, Clone, Eq, PartialEq, Debug)]
pub struct ChunkLocation {
    id: Key,
    start: u64,
    length: u64,
}

impl PartialOrd for ChunkLocation {
    fn partial_cmp(&self, other: &ChunkLocation) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ChunkLocation {
    fn cmp(&self, other: &ChunkLocation) -> Ordering {
        self.start.cmp(&other.start)
    }
}

#[derive(Serialize, Deserialize, Clone)]
/// An active Archive
pub struct Archive {
    name: String,
    objects: HashMap<String, Vec<ChunkLocation>>,
}

impl Archive {
    pub fn new(name: &str) -> Archive {
        Archive {
            name: name.to_string(),
            objects: HashMap::new(),
        }
    }

    #[cfg_attr(feature = "profile", flame)]
    pub fn put_object(
        &mut self,
        chunker: &Chunker,
        repository: &mut Repository,
        path: &str,
        from_reader: &mut Read,
    ) -> Option<()> {
        let mut locations: Vec<ChunkLocation> = Vec::new();

        #[cfg(feature = "profile")]
        flame::start("Packing chunks");
        let slices = chunker.chunked_iterator(from_reader);
        for Slice { data, start, end } in slices {
            let id = repository.write_chunk(&data)?;
            locations.push(ChunkLocation {
                id,
                start,
                length: end - start + 1,
            });
        }
        #[cfg(feature = "profile")]
        flame::end("Packing chunks");

        self.objects.insert(path.to_string(), locations);

        Some(())
    }

    #[cfg_attr(feature = "profile", flame)]
    pub fn get_object(
        &self,
        repository: &Repository,
        path: &str,
        restore_to: &mut Write,
    ) -> Option<()> {
        // Get chunk locations
        let mut locations = self.objects.get(&path.to_string())?.clone();
        locations.sort_unstable();
        let mut last_index = locations[0].start;
        for location in locations.iter() {
            let id = location.id;
            // If a chunk is not included, fill the space inbween it and the last with zeros
            let start = location.start;
            if start > last_index + 1 {
                let zero = [0_u8];
                for _ in last_index + 1..start {
                    restore_to.write(&zero).ok()?;
                }
            }
            let bytes = repository.read_chunk(id)?;

            restore_to.write_all(&bytes).ok()?;
            last_index = start + location.length - 1;
        }

        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::backend::filesystem::*;
    use crate::repository::compression::Compression;
    use crate::repository::encryption::Encryption;
    use crate::repository::hmac::HMAC;
    use quickcheck::quickcheck;
    use rand::prelude::*;
    use std::fs;
    use std::io::{BufReader, Cursor};
    use std::path::Path;
    use tempfile::tempdir;

    quickcheck! {
        fn single_add_get(seed: u64) -> bool {
            println!("Seed: {}", seed);
            let chunker = Chunker::new(8, 12, 0);

            let key: [u8; 32] = [0u8; 32];
            let size = 2 * 2_usize.pow(14);
            let mut data = vec![0_u8; size];
            let mut rand = SmallRng::seed_from_u64(seed);
            rand.fill_bytes(&mut data);

            let root_dir = tempdir().unwrap();
            let root_path = root_dir.path().display().to_string();

            let backend = Box::new(FileSystem::new_test(&root_path));
            let mut repo = Repository::new(
                backend,
                Compression::ZStd { level: 1 },
                HMAC::Blake2b,
                Encryption::new_aes256ctr(),
                &key,
            );

            let mut archive = Archive::new("test");

            let testdir = tempdir().unwrap();
            let input_file_path = testdir.path().join(Path::new("file1"));
            {
                let mut input_file = fs::File::create(input_file_path.clone()).unwrap();
                input_file.write_all(&data).unwrap();
            }
            let mut input_file = BufReader::new(fs::File::open(input_file_path).unwrap());

            archive.put_object(&chunker, &mut repo, "FileOne", &mut input_file);

            let mut buf = Cursor::new(Vec::<u8>::new());
            archive.get_object(&mut repo, "FileOne", &mut buf);

            let output = buf.into_inner();
            println!("Input length: {}", data.len());
            println!("Output length: {}", output.len());

            let mut mismatch = false;
            for i in 0..data.len() {
                if data[i] != output[i] {
                    println!(
                        "Byte {} was different in output. Input val: {:X?} Output val {:X?}",
                        i, data[i], output[i]
                    );

                    mismatch = true;
                }
            }

            !mismatch
        }
    }

}
