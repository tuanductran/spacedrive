use crate::{
	invalidate_query,
	node::Platform,
	prisma::{node, PrismaClient},
	sync::SyncManager,
	util::{
		db::{load_and_migrate, write_storedkey_to_db},
		seeder::{indexer_rules_seeder, SeederError},
	},
	NodeContext,
};

use sd_crypto::{
	keys::keymanager::{KeyManager, StoredKey},
	primitives::{to_array, OnboardingConfig},
};
use std::{
	env,
	path::{Path, PathBuf},
	str::FromStr,
	sync::Arc,
};
use thiserror::Error;
use tokio::{fs, io, sync::RwLock, try_join};
use uuid::Uuid;

use super::{LibraryConfig, LibraryConfigWrapped, LibraryContext};

/// LibraryManager is a singleton that manages all libraries for a node.
pub struct LibraryManager {
	/// libraries_dir holds the path to the directory where libraries are stored.
	libraries_dir: PathBuf,
	/// libraries holds the list of libraries which are currently loaded into the node.
	libraries: RwLock<Vec<LibraryContext>>,
	/// node_context holds the context for the node which this library manager is running on.
	node_context: NodeContext,
}

#[derive(Error, Debug)]
pub enum LibraryManagerError {
	#[error("error saving or loading the config from the filesystem")]
	IO(#[from] io::Error),
	#[error("error serializing or deserializing the JSON in the config file")]
	Json(#[from] serde_json::Error),
	#[error("Database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error("Library not found error")]
	LibraryNotFound,
	#[error("error migrating the config file")]
	Migration(String),
	#[error("failed to parse uuid")]
	Uuid(#[from] uuid::Error),
	#[error("error opening database as the path contains non-UTF-8 characters")]
	InvalidDatabasePath(PathBuf),
	#[error("Failed to run seeder: {0}")]
	Seeder(#[from] SeederError),
	#[error("failed to initialise the key manager")]
	KeyManager(#[from] sd_crypto::Error),
}

impl From<LibraryManagerError> for rspc::Error {
	fn from(error: LibraryManagerError) -> Self {
		rspc::Error::with_cause(
			rspc::ErrorCode::InternalServerError,
			error.to_string(),
			error,
		)
	}
}

pub async fn seed_keymanager(
	client: &PrismaClient,
	km: &Arc<KeyManager>,
) -> Result<(), LibraryManagerError> {
	let mut default = None;

	// collect and serialize the stored keys
	let stored_keys = client
		.key()
		.find_many(vec![])
		.exec()
		.await?
		.into_iter()
		.map(|key| {
			// SAFETY: This unwrap is alright as we generated the uuid for the key before
			let uuid = Uuid::from_str(&key.uuid).unwrap();

			if key.default {
				default = Some(uuid);
			}

			Ok(StoredKey {
				uuid,
				version: serde_json::from_str(&key.version)
					.map_err(|_| sd_crypto::Error::Serialization)?,
				algorithm: serde_json::from_str(&key.algorithm)
					.map_err(|_| sd_crypto::Error::Serialization)?,
				content_salt: to_array(key.content_salt)?,
				master_key: to_array(key.master_key)?,
				master_key_nonce: key.master_key_nonce,
				key_nonce: key.key_nonce,
				key: key.key,
				hashing_algorithm: serde_json::from_str(&key.hashing_algorithm)
					.map_err(|_| sd_crypto::Error::Serialization)?,
				salt: to_array(key.salt)?,
				memory_only: false,
				automount: key.automount,
			})
		})
		.collect::<Result<Vec<_>, sd_crypto::Error>>()?;

	// insert all keys from the DB into the keymanager's keystore
	km.populate_keystore(stored_keys).await?;

	// if any key had an associated default tag
	default.map(|k| km.set_default(k));

	Ok(())
}

impl LibraryManager {
	pub(crate) async fn new(
		libraries_dir: PathBuf,
		node_context: NodeContext,
	) -> Result<Self, LibraryManagerError> {
		fs::create_dir_all(&libraries_dir).await?;

		let mut libraries = Vec::new();
		let mut libraries_read_dir = fs::read_dir(&libraries_dir).await?;

		while let Some(entry) = libraries_read_dir.next_entry().await? {
			let config_path = entry.path();
			if entry.file_type().await?.is_file()
				&& config_path
					.extension()
					.map(|v| v == "sdlibrary")
					.unwrap_or(false)
			{
				let Some(Ok(library_id)) = config_path.file_stem().and_then(|stem| stem.to_str().map(Uuid::from_str)) else {
					println!(
						"Attempted to load library from path '{}' but it has \
						an invalid filename. Skipping...",
						config_path.display()
					);
					continue;
				};

				let db_path = config_path.with_extension("db");
				match fs::metadata(&db_path).await {
					Ok(_) => {}
					Err(e) if e.kind() == io::ErrorKind::NotFound => {
						println!(
							"Found library '{}' <id='{library_id}'> but no matching database file was found. Skipping...",
							config_path.display()
						);
						continue;
					}
					Err(e) => return Err(e.into()),
				}

				libraries.push(
					Self::load(
						library_id,
						&db_path,
						LibraryConfig::read(config_path).await?,
						node_context.clone(),
					)
					.await?,
				);
			}
		}

		Ok(Self {
			libraries: RwLock::new(libraries),
			libraries_dir,
			node_context,
		})
	}

	/// create creates a new library with the given config and mounts it into the running [LibraryManager].
	pub(crate) async fn create(
		&self,
		config: LibraryConfig,
		km_config: OnboardingConfig,
	) -> Result<LibraryConfigWrapped, LibraryManagerError> {
		let id = Uuid::new_v4();
		LibraryConfig::save(self.libraries_dir.join(format!("{id}.sdlibrary")), &config).await?;

		let library = Self::load(
			id,
			self.libraries_dir.join(format!("{id}.db")),
			config.clone(),
			self.node_context.clone(),
		)
		.await?;

		// Run seeders
		indexer_rules_seeder(&library.db).await?;

		// setup master password
		let verification_key = KeyManager::onboarding(km_config).await?;

		write_storedkey_to_db(&library.db, &verification_key).await?;

		// populate KM with the verification key
		seed_keymanager(&library.db, &library.key_manager).await?;

		invalidate_query!(library, "library.list");

		self.libraries.write().await.push(library);
		Ok(LibraryConfigWrapped { uuid: id, config })
	}

	pub(crate) async fn get_all_libraries_config(&self) -> Vec<LibraryConfigWrapped> {
		self.libraries
			.read()
			.await
			.iter()
			.map(|lib| LibraryConfigWrapped {
				config: lib.config.clone(),
				uuid: lib.id,
			})
			.collect()
	}

	pub(crate) async fn get_all_libraries_ctx(&self) -> Vec<LibraryContext> {
		self.libraries.read().await.clone()
	}

	pub(crate) async fn edit(
		&self,
		id: Uuid,
		name: Option<String>,
		description: Option<String>,
	) -> Result<(), LibraryManagerError> {
		// check library is valid
		let mut libraries = self.libraries.write().await;
		let library = libraries
			.iter_mut()
			.find(|lib| lib.id == id)
			.ok_or(LibraryManagerError::LibraryNotFound)?;

		// update the library
		if let Some(name) = name {
			library.config.name = name;
		}
		if let Some(description) = description {
			library.config.description = description;
		}

		LibraryConfig::save(
			self.libraries_dir.join(format!("{id}.sdlibrary")),
			&library.config,
		)
		.await?;

		invalidate_query!(library, "library.list");

		Ok(())
	}

	pub async fn delete_library(&self, id: Uuid) -> Result<(), LibraryManagerError> {
		let mut libraries = self.libraries.write().await;

		let library = libraries
			.iter()
			.find(|l| l.id == id)
			.ok_or(LibraryManagerError::LibraryNotFound)?;

		try_join!(
			fs::remove_file(self.libraries_dir.join(format!("{}.db", library.id))),
			fs::remove_file(self.libraries_dir.join(format!("{}.sdlibrary", library.id)))
		)?;

		invalidate_query!(library, "library.list");

		libraries.retain(|l| l.id != id);

		Ok(())
	}

	// get_ctx will return the library context for the given library id.
	pub(crate) async fn get_ctx(&self, library_id: Uuid) -> Option<LibraryContext> {
		self.libraries
			.read()
			.await
			.iter()
			.find(|lib| lib.id == library_id)
			.map(Clone::clone)
	}

	/// load the library from a given path
	pub(crate) async fn load(
		id: Uuid,
		db_path: impl AsRef<Path>,
		config: LibraryConfig,
		node_context: NodeContext,
	) -> Result<LibraryContext, LibraryManagerError> {
		let db_path = db_path.as_ref();
		let db = Arc::new(
			load_and_migrate(&format!(
				"file:{}",
				db_path.as_os_str().to_str().ok_or_else(|| {
					LibraryManagerError::InvalidDatabasePath(db_path.to_path_buf())
				})?
			))
			.await
			.unwrap(),
		);

		let node_config = node_context.config.get().await;

		let platform = match env::consts::OS {
			"windows" => Platform::Windows,
			"macos" => Platform::MacOS,
			"linux" => Platform::Linux,
			_ => Platform::Unknown,
		};

		let uuid_vec = id.as_bytes().to_vec();
		let node_data = db
			.node()
			.upsert(
				node::pub_id::equals(uuid_vec.clone()),
				(
					uuid_vec,
					node_config.name.clone(),
					vec![node::platform::set(platform as i32)],
				),
				vec![node::name::set(node_config.name.clone())],
			)
			.exec()
			.await?;

		let key_manager = Arc::new(KeyManager::new(vec![]).await?);

		seed_keymanager(&db, &key_manager).await?;
		let (sync_manager, _) = SyncManager::new(db.clone(), id);

		Ok(LibraryContext {
			id,
			local_id: node_data.id,
			config,
			key_manager,
			sync: Arc::new(sync_manager),
			db,
			node_local_id: node_data.id,
			node_context,
		})
	}
}
