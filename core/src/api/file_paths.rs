use crate::{
	api::utils::library,
	invalidate_query,
	library::Library,
	location::{
		file_path_helper::{
			file_path_to_isolate, file_path_to_isolate_with_id, FilePathError, IsolatedFilePathData,
		},
		find_location, LocationError,
	},
	object::fs::{
		copy::FileCopierJobInit, cut::FileCutterJobInit, delete::FileDeleterJobInit,
		erase::FileEraserJobInit,
	},
	prisma::{file_path, location},
};

use std::path::Path;

use futures::future::try_join_all;
use regex::Regex;
use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::Deserialize;
use specta::Type;
use tokio::fs;
use tracing::error;

use super::{Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		// .procedure("encryptFiles", {
		// 	R.with2(library())
		// 		.mutation(|(_, library), args: FileEncryptorJobInit| async move {
		// 			library.spawn_job(args).await.map_err(Into::into)
		// 		})
		// })
		// .procedure("decryptFiles", {
		// 	R.with2(library())
		// 		.mutation(|(_, library), args: FileDecryptorJobInit| async move {
		// 			library.spawn_job(args).await.map_err(Into::into)
		// 		})
		// })
		.procedure("delete", {
			R.with2(library())
				.mutation(|(_, library), args: FileDeleterJobInit| async move {
					library.spawn_job(args).await.map_err(Into::into)
				})
		})
		.procedure("erase", {
			R.with2(library())
				.mutation(|(_, library), args: FileEraserJobInit| async move {
					library.spawn_job(args).await.map_err(Into::into)
				})
		})
		.procedure("duplicate", {
			R.with2(library())
				.mutation(|(_, library), args: FileCopierJobInit| async move {
					library.spawn_job(args).await.map_err(Into::into)
				})
		})
		.procedure("copy", {
			R.with2(library())
				.mutation(|(_, library), args: FileCopierJobInit| async move {
					library.spawn_job(args).await.map_err(Into::into)
				})
		})
		.procedure("cut", {
			R.with2(library())
				.mutation(|(_, library), args: FileCutterJobInit| async move {
					library.spawn_job(args).await.map_err(Into::into)
				})
		})
		.procedure("rename", {
			#[derive(Type, Deserialize)]
			pub struct FromPattern {
				pub pattern: String,
				pub replace_all: bool,
			}

			#[derive(Type, Deserialize)]
			pub struct RenameOne {
				pub from_file_path_id: file_path::id::Type,
				pub to: String,
			}

			#[derive(Type, Deserialize)]
			pub struct RenameMany {
				pub from_pattern: FromPattern,
				pub to_pattern: String,
				pub from_file_path_ids: Vec<file_path::id::Type>,
			}

			#[derive(Type, Deserialize)]
			pub enum RenameKind {
				One(RenameOne),
				Many(RenameMany),
			}

			#[derive(Type, Deserialize)]
			pub struct RenameFileArgs {
				pub location_id: location::id::Type,
				pub kind: RenameKind,
			}

			impl RenameFileArgs {
				pub async fn rename_one(
					RenameOne {
						from_file_path_id,
						to,
					}: RenameOne,
					location_path: impl AsRef<Path>,
					library: &Library,
				) -> Result<(), rspc::Error> {
					let location_path = location_path.as_ref();
					let iso_file_path = IsolatedFilePathData::try_from(
						library
							.db
							.file_path()
							.find_unique(file_path::id::equals(from_file_path_id))
							.select(file_path_to_isolate::select())
							.exec()
							.await?
							.ok_or(LocationError::FilePath(FilePathError::IdNotFound(
								from_file_path_id,
							)))?,
					)
					.map_err(LocationError::MissingField)?;

					if iso_file_path.full_name() == to {
						return Ok(());
					}

					let (new_file_name, new_extension) =
						IsolatedFilePathData::separate_name_and_extension_from_str(&to)
							.map_err(LocationError::FilePath)?;

					let mut new_file_full_path = location_path.join(iso_file_path.parent());
					new_file_full_path.push(new_file_name);
					if !new_extension.is_empty() {
						new_file_full_path.set_extension(new_extension);
					}

					match fs::metadata(&new_file_full_path).await {
						Ok(_) => {
							return Err(rspc::Error::new(
								ErrorCode::Conflict,
								"File already exists".to_string(),
							))
						}
						Err(e) => {
							if e.kind() != std::io::ErrorKind::NotFound {
								return Err(rspc::Error::with_cause(
									ErrorCode::InternalServerError,
									"Failed to check if file exists".to_string(),
									e,
								));
							}
						}
					}

					fs::rename(location_path.join(&iso_file_path), new_file_full_path)
						.await
						.map_err(|e| {
							rspc::Error::with_cause(
								ErrorCode::Conflict,
								"Failed to rename file".to_string(),
								e,
							)
						})?;

					library
						.db
						.file_path()
						.update(
							file_path::id::equals(from_file_path_id),
							vec![
								file_path::name::set(Some(new_file_name.to_string())),
								file_path::extension::set(Some(new_extension.to_string())),
							],
						)
						.exec()
						.await?;

					Ok(())
				}

				pub async fn rename_many(
					RenameMany {
						from_pattern,
						to_pattern,
						from_file_path_ids,
					}: RenameMany,
					location_path: impl AsRef<Path>,
					library: &Library,
				) -> Result<(), rspc::Error> {
					let location_path = location_path.as_ref();

					let Ok(from_regex) = Regex::new(&from_pattern.pattern) else {
						return Err(rspc::Error::new(
							rspc::ErrorCode::BadRequest,
							"Invalid `from` regex pattern".into(),
						));
					};

					let to_update = try_join_all(
						library
							.db
							.file_path()
							.find_many(vec![file_path::id::in_vec(from_file_path_ids)])
							.select(file_path_to_isolate_with_id::select())
							.exec()
							.await?
							.into_iter()
							.flat_map(|file_path| {
								let id = file_path.id;

								IsolatedFilePathData::try_from(file_path).map(|d| (id, d))
							})
							.map(|(file_path_id, iso_file_path)| {
								let from = location_path.join(&iso_file_path);
								let mut to = location_path.join(iso_file_path.parent());
								let full_name = iso_file_path.full_name();
								let replaced_full_name = if from_pattern.replace_all {
									from_regex.replace_all(&full_name, &to_pattern)
								} else {
									from_regex.replace(&full_name, &to_pattern)
								}
								.to_string();

								to.push(&replaced_full_name);

								async move {
									if !IsolatedFilePathData::accept_file_name(&replaced_full_name)
									{
										Err(rspc::Error::new(
											ErrorCode::BadRequest,
											"Invalid file name".to_string(),
										))
									} else {
										fs::rename(&from, &to)
											.await
											.map_err(|e| {
												error!(
													"Failed to rename file from: '{}' to: '{}'",
													from.display(),
													to.display()
												);
												rspc::Error::with_cause(
													ErrorCode::Conflict,
													"Failed to rename file".to_string(),
													e,
												)
											})
											.map(|_| {
												let (name, extension) =
												IsolatedFilePathData::separate_name_and_extension_from_str(
												&replaced_full_name,
												)
												.expect("we just built this full name and validated it");

												(
													file_path_id,
													(name.to_string(), extension.to_string()),
												)
											})
									}
								}
							}),
					)
					.await?;

					// TODO: dispatch sync update events

					library
						.db
						._batch(
							to_update
								.into_iter()
								.map(|(file_path_id, (new_name, new_extension))| {
									library.db.file_path().update(
										file_path::id::equals(file_path_id),
										vec![
											file_path::name::set(Some(new_name)),
											file_path::extension::set(Some(new_extension)),
										],
									)
								})
								.collect::<Vec<_>>(),
						)
						.await?;

					Ok(())
				}
			}

			R.with2(library())
				.mutation(|(_, library), args: RenameFileArgs| async move {
					let location_path = find_location(&library, args.location_id)
						.select(location::select!({ path }))
						.exec()
						.await?
						.ok_or(LocationError::IdNotFound(args.location_id))?
						.path
						.ok_or(LocationError::MissingPath(args.location_id))?;

					let res = match args.kind {
						RenameKind::One(one) => {
							RenameFileArgs::rename_one(one, location_path, &library).await
						}
						RenameKind::Many(many) => {
							RenameFileArgs::rename_many(many, location_path, &library).await
						}
					};

					invalidate_query!(library, "search.objects");

					res
				})
		})
}
