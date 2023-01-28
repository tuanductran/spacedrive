use std::path::{Path, PathBuf};

use rspc::Type;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{
	fs, io,
	sync::{RwLock, RwLockWriteGuard},
};
use tracing::error;
use uuid::Uuid;

/// NODE_STATE_CONFIG_NAME is the name of the file which stores the NodeState
pub const NODE_STATE_CONFIG_NAME: &str = "node_state.sdconfig";

/// ConfigMetadata is a part of node configuration that is loaded before the main configuration and contains information about the schema of the config.
/// This allows us to migrate breaking changes to the config format between Spacedrive releases.
#[derive(Debug, Serialize, Deserialize, Clone, Type, PartialEq, Eq)]
pub struct ConfigMetadata {
	/// version of Spacedrive. Determined from `CARGO_PKG_VERSION` environment variable.
	pub version: Option<String>,
}

impl Default for ConfigMetadata {
	fn default() -> Self {
		Self {
			version: Some(env!("CARGO_PKG_VERSION").into()),
		}
	}
}

/// NodeConfig is the configuration for a node. This is shared between all libraries and is stored in a JSON file on disk.
#[derive(Debug, Serialize, Deserialize, Clone, Type)]
pub struct NodeConfig {
	#[serde(flatten)]
	pub metadata: ConfigMetadata,
	/// id is a unique identifier for the current node. Each node has a public identifier (this one) and is given a local id for each library (done within the library code).
	pub id: Uuid,
	/// name is the display name of the current node. This is set by the user and is shown in the UI. // TODO: Length validation so it can fit in DNS record
	pub name: String,
	// the port this node uses for peer to peer communication. By default a random free port will be chosen each time the application is started.
	pub p2p_port: Option<u32>,
	// /// The P2P identity public key
	// pub p2p_cert: Vec<u8>,
	// /// The P2P identity private key
	// pub p2p_key: Vec<u8>,
	// /// The address of the Spacetunnel discovery service being used.
	// pub spacetunnel_addr: Option<String>,
}

#[derive(Error, Debug)]
pub enum NodeConfigError {
	#[error("error saving or loading the config from the filesystem: {0}")]
	IO(#[from] io::Error),
	#[error("error serializing or deserializing the JSON in the config file: {0}")]
	Json(#[from] serde_json::Error),
	#[error("error migrating the config file: {0}")]
	Migration(String),
}

impl NodeConfig {
	fn default() -> Self {
		NodeConfig {
			id: Uuid::new_v4(),
			name: hostname::get()
				// SAFETY: This is just for display purposes so it doesn't matter if it's lossy
				.map(|hostname| hostname.to_string_lossy().into_owned())
				.unwrap_or_else(|err| {
					error!(
						"Falling back to default node name as an error \
						occurred getting your systems hostname: '{err}'"
					);
					"my-spacedrive".into()
				}),
			p2p_port: None,
			metadata: Default::default(),
		}
	}
}

pub struct NodeConfigManager {
	config: RwLock<NodeConfig>,
	data_directory: PathBuf,
	config_filepath: PathBuf,
}

impl NodeConfigManager {
	/// new will create a new NodeConfigManager with the given path to the config file.
	pub(crate) async fn new(data_directory: impl AsRef<Path>) -> Result<Self, NodeConfigError> {
		let data_directory = data_directory.as_ref().to_path_buf();
		Ok(Self {
			config: RwLock::new(Self::read(&data_directory).await?),
			config_filepath: data_directory.join(NODE_STATE_CONFIG_NAME),
			data_directory,
		})
	}

	/// get will return the current NodeConfig in a read only state.
	pub(crate) async fn get(&self) -> NodeConfig {
		self.config.read().await.clone()
	}

	/// data_directory returns the path to the directory storing the configuration data.
	pub(crate) fn data_directory(&self) -> PathBuf {
		self.data_directory.clone()
	}

	/// write allows the user to update the configuration. This is done in a closure while a Mutex lock is held so that the user can't cause a race condition if the config were to be updated in multiple parts of the app at the same time.
	#[allow(unused)]
	pub(crate) async fn write<F: FnOnce(RwLockWriteGuard<NodeConfig>)>(
		&self,
		mutation_fn: F,
	) -> Result<NodeConfig, NodeConfigError> {
		mutation_fn(self.config.write().await);
		let config = self.config.read().await;
		Self::save_to_file(&self.config_filepath, &config).await?;
		Ok(config.clone())
	}

	/// read will read the configuration from disk and return it.
	async fn read(config_filepath: impl AsRef<Path>) -> Result<NodeConfig, NodeConfigError> {
		let config_filepath = config_filepath.as_ref();

		match fs::metadata(config_filepath).await {
			Ok(_) => {
				// TODO: In the future use a async `from_reader` in serde_json to partially read from disk
				let mut config =
					serde_json::from_slice::<NodeConfig>(&fs::read(config_filepath).await?)?;

				if Self::migrate_config(&config.metadata, config_filepath).await? {
					// Reloading config, as we had to migrate the config file
					config =
						serde_json::from_slice::<NodeConfig>(&fs::read(config_filepath).await?)?;
				}

				Ok(config)
			}
			Err(e) if e.kind() == io::ErrorKind::NotFound => {
				let config = NodeConfig::default();
				Self::save_to_file(config_filepath, &config).await?;
				Ok(config)
			}
			Err(e) => return Err(e.into()),
		}
	}

	/// Inner static method to actually save a config in disk
	async fn save_to_file(
		config_filepath: impl AsRef<Path>,
		config: &NodeConfig,
	) -> Result<(), NodeConfigError> {
		fs::write(config_filepath, serde_json::to_vec(config)?)
			.await
			.map_err(Into::into)
	}

	/// Migrate_config is a function used to apply breaking changes to the config file.
	async fn migrate_config(
		current_config_metadata: &ConfigMetadata,
		config_filepath: impl AsRef<Path>,
	) -> Result<bool, NodeConfigError> {
		// If the received version is the default one, so we don't need to migrate the config file
		if current_config_metadata == &ConfigMetadata::default() {
			return Ok(false);
		}

		match current_config_metadata.version {
			None => Err(NodeConfigError::Migration(format!(
				"Your Spacedrive config file stored at '{}' is missing the `version` \
					field. If you just upgraded please delete the file and restart Spacedrive! \
					Please note this upgrade will stop using your old 'library.db' as the \
					folder structure has changed.",
				config_filepath.as_ref().display()
			))),
			// TODO: When we need a config migration, fill in for a new match arm with Some("version")
			_ => Ok(true),
		}
	}
}
