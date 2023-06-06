use crate::{
	library::Library,
	location::indexer::IndexerError,
	object::{file_identifier::FileIdentifierJobError, preview::ThumbnailerError},
	util::error::FileIOError,
};

use std::{
	collections::{hash_map::DefaultHasher, VecDeque},
	fmt::Debug,
	hash::{Hash, Hasher},
	mem,
	path::PathBuf,
	sync::Arc,
};

use async_stream::stream;
use futures::Stream;
use rmp_serde::{decode::Error as DecodeError, encode::Error as EncodeError};
use sd_crypto::Error as CryptoError;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

mod job_manager;
mod worker;

pub use job_manager::*;
pub use worker::*;

pub enum JobMessage {
	//
	// Error(()),
	// Yielding this will mark the job as complete and stop it being polled
	Complete,
}

/// TODO
pub trait Job: Serialize + DeserializeOwned + Hash {
	/// The name of the job is a unique human readable identifier for the job.
	const NAME: &'static str;
	const IS_BACKGROUND: bool = false;

	// TODO: Arc<Library> everywhere!!!!
	fn run(&mut self, library: Library) -> Result<Box<dyn Stream<Item = JobMessage>>, JobError>;
}

// TODO: Pause job
// TODO: Shutdown

#[derive(Error, Debug)]
pub enum JobError {
	// General errors
	#[error("database error")]
	DatabaseError(#[from] prisma_client_rust::QueryError),
	#[error("Failed to join Tokio spawn blocking: {0}")]
	JoinTaskError(#[from] tokio::task::JoinError),
	#[error("Job state encode error: {0}")]
	StateEncode(#[from] EncodeError),
	#[error("Job state decode error: {0}")]
	StateDecode(#[from] DecodeError),
	#[error("Job metadata serialization error: {0}")]
	MetadataSerialization(#[from] serde_json::Error),
	#[error("Tried to resume a job with unknown name: job <name='{1}', uuid='{0}'>")]
	UnknownJobName(Uuid, String),
	#[error(
		"Tried to resume a job that doesn't have saved state data: job <name='{1}', uuid='{0}'>"
	)]
	MissingJobDataState(Uuid, String),
	#[error("missing report field: job <uuid='{id}', name='{name}'>")]
	MissingReport { id: Uuid, name: String },
	#[error("missing some job data: '{value}'")]
	MissingData { value: String },
	#[error("error converting/handling OS strings")]
	OsStr,
	#[error("error converting/handling paths")]
	Path,
	#[error("invalid job status integer")]
	InvalidJobStatusInt(i32),
	#[error(transparent)]
	FileIO(#[from] FileIOError),

	// Specific job errors
	#[error("Indexer error: {0}")]
	IndexerError(#[from] IndexerError),
	#[error("Thumbnailer error: {0}")]
	ThumbnailError(#[from] ThumbnailerError),
	#[error("Identifier error: {0}")]
	IdentifierError(#[from] FileIdentifierJobError),
	#[error("Crypto error: {0}")]
	CryptoError(#[from] CryptoError),
	#[error("source and destination path are the same: {}", .0.display())]
	MatchingSrcDest(PathBuf),
	#[error("action would overwrite another file: {}", .0.display())]
	WouldOverwrite(PathBuf),
	#[error("item of type '{0}' with id '{1}' is missing from the db")]
	MissingFromDb(&'static str, String),
	#[error("the cas id is not set on the path data")]
	MissingCasId,

	// Not errors
	#[error("step completed with errors")]
	StepCompletedWithErrors(JobRunErrors),
	#[error("job had a early finish: <name='{name}', reason='{reason}'>")]
	EarlyFinish { name: String, reason: String },
	#[error("data needed for job execution not found: job <name='{0}'>")]
	JobDataNotFound(String),
	#[error("job paused")]
	Paused(Vec<u8>),
}
