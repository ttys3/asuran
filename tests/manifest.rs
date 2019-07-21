use libasuran::chunker::*;
use libasuran::manifest::*;
use libasuran::repository::*;
use rand::prelude::*;
use std::io::Cursor;
use tempfile::tempdir;

mod common;

#[test]
fn put_drop_get() {
    let tempdir = tempdir().unwrap();
    let root_path = tempdir.path().to_str().unwrap();
    let key = Key::random(32);
    let mut repo = common::get_repo(root_path, key);
    let chunker = Chunker::new(48, 12, 0);

    let mut objects: Vec<Vec<u8>> = Vec::new();

    for _ in 0..5 {
        let mut object = vec![0_u8; 16384];
        thread_rng().fill_bytes(&mut object);
        objects.push(object);
    }

    {
        let mut manifest = Manifest::empty_manifest(repo.chunk_settings());
        manifest.commit(&mut repo);
        let mut archive = Archive::new("test");
        for (i, object) in objects.iter().enumerate() {
            archive.put_object(
                &chunker,
                &mut repo,
                &i.to_string(),
                &mut Cursor::new(object),
            );
        }
        println!("Archive: \n {:?}", archive);
        manifest.commit_archive(&mut repo, archive);
        println!("Manifest: \n {:?}", manifest);
    }

    let manifest = Manifest::load(&repo);
    let archive = manifest.archives()[0].load(&repo).unwrap();
    for (i, object) in objects.iter().enumerate() {
        let mut buffer = Cursor::new(Vec::<u8>::new());
        println!("Archive: \n {:?}", archive);
        archive
            .get_object(&repo, &i.to_string(), &mut buffer)
            .unwrap();
        let buffer = buffer.into_inner();
        assert_eq!(object, &buffer);
    }
}
