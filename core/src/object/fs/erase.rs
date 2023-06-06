use crate::{
	invalidate_query,
	job::{Job, JobError, JobMessage, JobReportUpdate},
	library::Library,
	util::error::FileIOError,
};

use std::{hash::Hash, path::PathBuf};

use async_stream::stream;
use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use specta::Type;
use tokio::{fs::OpenOptions, io::AsyncWriteExt};
use tracing::trace;

use super::{context_menu_fs_info, FsInfo};

#[derive(Serialize, Deserialize, Hash, Type)]
pub enum FileEraserJob {
	Init(FileEraserJobInit),
	Run(FsInfo),
}

#[serde_as]
#[derive(Serialize, Deserialize, Hash, Type)]
pub struct FileEraserJobInit {
	pub location_id: i32,
	pub path_id: i32,
	#[specta(type = String)]
	#[serde_as(as = "DisplayFromStr")]
	pub passes: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FileEraserJobStep {
	Directory { path: PathBuf },
	File { path: PathBuf },
}

impl From<FsInfo> for FileEraserJobStep {
	fn from(value: FsInfo) -> Self {
		if value.path_data.is_dir {
			Self::Directory {
				path: value.fs_path,
			}
		} else {
			Self::File {
				path: value.fs_path,
			}
		}
	}
}

impl Job for FileEraserJob {
	const NAME: &'static str = "file_eraser";
	const IS_BACKGROUND: bool = false;

	fn run(&mut self, library: Library) -> Result<Box<dyn Stream<Item = JobMessage>>, JobError> {
		stream! {
			loop {
				match *self {
					FileEraserJob::Init(init) => {
						let fs_info =
							context_menu_fs_info(&library.db, init.location_id, init.path_id)
								.await?;

						*self = Self::Run(fs_info);
					},
					FileEraserJob::Run(data) => {
						// need to handle stuff such as querying prisma for all paths of a file, and deleting all of those if requested (with a checkbox in the ui)
						// maybe a files.countOccurances/and or files.getPath(location_id, path_id) to show how many of these files would be erased (and where?)

		match &data {
			FileEraserJobStep::File { path } => {
				let mut file = OpenOptions::new()
					.read(true)
					.write(true)
					.open(path)
					.await
					.map_err(|e| FileIOError::from((path, e)))?;
				let file_len = file
					.metadata()
					.await
					.map_err(|e| FileIOError::from((path, e)))?
					.len();

				sd_crypto::fs::erase::erase(&mut file, file_len as usize, state.init.passes)
					.await?;

				file.set_len(0)
					.await
					.map_err(|e| FileIOError::from((path, e)))?;
				file.flush()
					.await
					.map_err(|e| FileIOError::from((path, e)))?;
				drop(file);

				trace!("Erasing file: {:?}", path);

				tokio::fs::remove_file(path)
					.await
					.map_err(|e| FileIOError::from((path, e)))?;
			}
			FileEraserJobStep::Directory { path } => {
				let path = path.clone(); // To appease the borrowck

				let mut dir = tokio::fs::read_dir(&path)
					.await
					.map_err(|e| FileIOError::from((&path, e)))?;

				while let Some(entry) = dir
					.next_entry()
					.await
					.map_err(|e| FileIOError::from((&path, e)))?
				{
					let entry_path = entry.path();
					state.steps.push_back(
						if entry
							.metadata()
							.await
							.map_err(|e| FileIOError::from((&entry_path, e)))?
							.is_dir()
						{
							FileEraserJobStep::Directory { path: entry_path }
						} else {
							FileEraserJobStep::File { path: entry_path }
						},
					);

					ctx.progress(vec![JobReportUpdate::TaskCount(state.steps.len())]);
				}
			}
		};

		// TODO
		ctx.progress(vec![JobReportUpdate::CompletedTaskCount(
			state.step_number + 1,
		)]);



						todo!();
					}
				}
			}
		}
	}
}

// 	async fn finalize(&mut self, ctx: WorkerContext, state: &mut JobState<Self>) -> JobResult {
// 		let data = state
// 			.data
// 			.as_ref()
// 			.expect("critical error: missing data on job state");
// 		if data.path_data.is_dir {
// 			tokio::fs::remove_dir_all(&data.fs_path)
// 				.await
// 				.map_err(|e| FileIOError::from((&data.fs_path, e)))?;
// 		}

// 		invalidate_query!(ctx.library, "search.paths");

// 		Ok(Some(serde_json::to_value(&state.init)?))
// 	}
// }
