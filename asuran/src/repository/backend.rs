//! The backend provides abstract IO access to the real location of the data in
//! the repository.
#![allow(clippy::used_underscore_binding)] // TODO: Fix this after clippy and thiserror start
                                           // playing nice
use crate::manifest::StoredArchive;
use crate::repository::{Chunk, ChunkID, ChunkSettings, EncryptedKey};

use async_trait::async_trait;
use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use std::collections::HashSet;

pub mod common;
pub mod flatfile;
pub mod mem;
pub mod multifile;
#[cfg(feature = "sftp")]
pub mod sftp;

#[cfg_attr(tarpaulin, skip)]
pub mod object_wrappers;
pub use object_wrappers::{backend_to_object, BackendObject};

/// An error for things that can go wrong with backends
#[derive(Error, Debug)]
pub enum BackendError {
    #[error("I/O Error: {0}")]
    IOError(#[from] std::io::Error),
    #[error("Data not found")]
    DataNotFound,
    #[error("Segment Error: {0}")]
    SegmentError(String),
    #[error("Manifest Error: {0}")]
    ManifestError(String),
    #[error("Index Error: {0}")]
    IndexError(String),
    #[error("MessagePack Decode Error")]
    MsgPackDecodeError(#[from] rmp_serde::decode::Error),
    #[error("MessagePack Encode Error")]
    MsgPackEncodeError(#[from] rmp_serde::encode::Error),
    #[error("Failed to lock file")]
    FileLockError,
    #[error("Cancelled oneshot")]
    CancelledOneshotError(#[from] futures::channel::oneshot::Canceled),
    #[error("Chunk Unpacking Error: {0}")]
    ChunkUnpackError(#[from] asuran_core::repository::chunk::ChunkError),
    #[error("Repository has an existing global lock: {0}")]
    RepositoryGloballyLocked(String),
    #[error("Task Communication Error, likely trying to talk to a closed backend")]
    ChannelDroppedSend(#[from] futures::channel::mpsc::SendError),
    #[error("Error connecting to backend: {0}")]
    ConnectionError(String),
    #[error("FlatFile Format Error: {0}")]
    FlatFile(#[from] asuran_core::repository::backend::flatfile::FlatFileError),
    #[error("Unknown Error: {0}")]
    Unknown(String),
}
pub type Result<T> = std::result::Result<T, BackendError>;

/// Describes the segment id and location there in of a chunk
///
/// This does not store the length, as segments are responsible for storing chunks
/// in a format that does not require prior knowledge of the chunk length.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct SegmentDescriptor {
    pub segment_id: u64,
    pub start: u64,
}

/// Manifest trait
///
/// Keeps track of which archives are in the repository.
///
/// All writing methods should commit to hard storage prior to returning
#[async_trait]
pub trait Manifest: Send + Sync + std::fmt::Debug + 'static {
    type Iterator: Iterator<Item = StoredArchive> + 'static;
    /// Timestamp of the last modification
    async fn last_modification(&mut self) -> Result<DateTime<FixedOffset>>;
    /// Returns the default settings for new chunks in this repository
    async fn chunk_settings(&mut self) -> ChunkSettings;
    /// Returns an iterator over the list of archives in this repository, in reverse chronological
    /// order (newest first).
    async fn archive_iterator(&mut self) -> Self::Iterator;

    /// Sets the chunk settings in the repository
    async fn write_chunk_settings(&mut self, settings: ChunkSettings) -> Result<()>;
    /// Adds an archive to the manifest
    async fn write_archive(&mut self, archive: StoredArchive) -> Result<()>;
    /// Updates the timestamp without performing any other operations
    async fn touch(&mut self) -> Result<()>;
}

/// Index Trait
///
/// Keeps track of where chunks are in the backend
#[async_trait]
pub trait Index: Send + Sync + std::fmt::Debug + 'static {
    /// Provides the location of a chunk in the repository
    async fn lookup_chunk(&mut self, id: ChunkID) -> Option<SegmentDescriptor>;
    /// Sets the location of a chunk in the repository
    async fn set_chunk(&mut self, id: ChunkID, location: SegmentDescriptor) -> Result<()>;
    /// Returns the set of all `ChunkID`s known to exist in the Asuran repository.
    async fn known_chunks(&mut self) -> HashSet<ChunkID>;
    /// Commits the index
    async fn commit_index(&mut self) -> Result<()>;
    /// Returns the total number of chunks in the index
    async fn count_chunk(&mut self) -> usize;
}

/// Repository backend
///
/// The backend handles the heavy lifiting of the IO, abstracting the repository
/// struct itself away from the details of the system used to store the
/// repository.
///
/// While the backend trait itself does not require `Clone`, most uses will
/// require that Backends be `Clone`, as expressed by the `BackendClone` trait.
///
/// `Backend` itself can not require clone, for object saftey
#[async_trait]
pub trait Backend: 'static + Send + Sync + std::fmt::Debug + 'static {
    type Manifest: Manifest + 'static;
    type Index: Index + 'static;
    /// Returns a view of the index of the repository
    fn get_index(&self) -> Self::Index;
    /// Writes the specified encrypted key to the backend
    ///
    /// Returns Err if the key could not be written
    async fn write_key(&self, key: &EncryptedKey) -> Result<()>;
    /// Attempts to read the encrypted key from the backend.
    async fn read_key(&self) -> Result<EncryptedKey>;
    /// Returns a view of this respository's manifest
    fn get_manifest(&self) -> Self::Manifest;
    /// Starts reading a chunk from the backend
    ///
    /// The chunk will be written to the oneshot once reading is complete
    async fn read_chunk(&mut self, location: SegmentDescriptor) -> Result<Chunk>;
    /// Starts writing a chunk to the backend
    ///
    /// A segment descriptor describing it will be written to oneshot once reading is complete
    ///
    /// This must be passed owned data because it will be sent into a task, so the caller has no
    /// control over drop time
    async fn write_chunk(&mut self, chunk: Chunk) -> Result<SegmentDescriptor>;
    /// Consumes the current backend handle, and does any work necessary to
    /// close out the backend properly
    ///
    /// This is separate from Drop due to the current lack of async drop
    ///
    /// This method takes &mut self such that it can be called on trait objects.
    /// It is not correct to call any methods on a Backend after close has
    /// returned
    async fn close(&mut self);
    /// Creates a new trait-object based BackendHandle
    ///
    /// This is required to implement clone for
    fn get_object_handle(&self) -> BackendObject;
}

pub trait BackendClone: Backend + Clone {}

impl<T: ?Sized> BackendClone for T where T: Backend + Clone {}

#[derive(Copy, PartialEq, Eq, Clone, Serialize, Deserialize, Debug)]
pub enum TransactionType {
    Insert,
    Delete,
}
