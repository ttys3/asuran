use libasuran::repository::*;

pub fn get_repo(root_path: &str, key: &[u8; 32]) -> Repository<impl Backend> {
    let backend = FileSystem::new(&root_path);
    Repository::new(
        backend,
        Compression::ZStd { level: 1 },
        HMAC::Blake2b,
        Encryption::new_aes256ctr(),
        key,
    )
}