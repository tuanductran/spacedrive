use crate::node::ConfigMetadata;

use std::path::Path;

use rspc::Type;
use serde::{Deserialize, Serialize};
use tokio::fs;
use uuid::Uuid;

use super::LibraryManagerError;

/// LibraryConfig holds the configuration for a specific library. This is stored as a '{uuid}.sdlibrary' file.
#[derive(Debug, Serialize, Deserialize, Clone, Type, Default)]
pub struct LibraryConfig {
	#[serde(flatten)]
	pub metadata: ConfigMetadata,
	/// name is the display name of the library. This is used in the UI and is set by the user.
	pub name: String,
	/// description is a user set description of the library. This is used in the UI and is set by the user.
	pub description: String,
	// /// is_encrypted is a flag that is set to true if the library is encrypted.
	// #[serde(default)]
	// pub is_encrypted: bool,
}

impl LibraryConfig {
	/// read will read the configuration from disk and return it.
	pub(super) async fn read(
		config_path: impl AsRef<Path>,
	) -> Result<LibraryConfig, LibraryManagerError> {
		let config_path = config_path.as_ref();

		// TODO: In the future use a async `from_reader` in serde_json to partially read from disk
		Self::migrate_config(
			&serde_json::from_slice::<ConfigMetadata>(&fs::read(config_path).await?)?,
			config_path,
		)
		.await?;

		serde_json::from_slice(&fs::read(config_path).await?).map_err(Into::into)
	}

	/// save will write the configuration back to disk
	pub(super) async fn save(
		config_path: impl AsRef<Path>,
		config: &LibraryConfig,
	) -> Result<(), LibraryManagerError> {
		fs::write(config_path, serde_json::to_vec(config)?)
			.await
			.map_err(Into::into)
	}

	/// migrate_config is a function used to apply breaking changes to the library config file.
	async fn migrate_config(
		current_config_metadata: &ConfigMetadata,
		config_path: impl AsRef<Path>,
	) -> Result<(), LibraryManagerError> {
		// If the received version is the default one, so we don't need to migrate the config file
		if current_config_metadata == &ConfigMetadata::default() {
			return Ok(());
		}

		match current_config_metadata.version {
			None => Err(LibraryManagerError::Migration(format!(
				"Your Spacedrive library at '{}' is missing the `version` field",
				config_path.as_ref().display()
			))),
			// TODO: When we need a config migration, fill in for a new match arm with Some("version")
			_ => Ok(()),
		}
	}
}

// used to return to the frontend with uuid context
#[derive(Serialize, Deserialize, Debug, Type)]
pub struct LibraryConfigWrapped {
	pub uuid: Uuid,
	pub config: LibraryConfig,
}
