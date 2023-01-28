use std::{path::Path, sync::Arc};

use thiserror::Error;
use tokio::{
	fs, io,
	sync::{broadcast, mpsc, RwLock},
};
use tracing::{error, info};
use tracing_subscriber::{prelude::*, EnvFilter};

pub mod api;
pub(crate) mod job;
pub(crate) mod library;
pub(crate) mod location;
pub(crate) mod node;
pub(crate) mod object;
pub(crate) mod sync;
pub(crate) mod util;
pub(crate) mod volume;

pub(crate) mod prisma;
pub(crate) mod prisma_sync;

use api::{CoreEvent, Ctx, Router};
use job::JobManager;
use library::LibraryManager;
use location::{LocationManager, LocationManagerError};
use node::{NodeConfigManager, NodeReboot};

#[derive(Clone)]
pub struct NodeContext {
	pub config: Arc<NodeConfigManager>,
	pub jobs: Arc<JobManager>,
	pub location_manager: Arc<LocationManager>,
	pub event_bus_tx: broadcast::Sender<CoreEvent>,
}

pub struct Node {
	config: Arc<NodeConfigManager>,
	library_manager: Arc<RwLock<Arc<LibraryManager>>>,
	jobs: Arc<RwLock<Arc<JobManager>>>,
	event_bus: (broadcast::Sender<CoreEvent>, broadcast::Receiver<CoreEvent>),
	reboot_tx: mpsc::Sender<NodeReboot>,
}

#[cfg(not(feature = "android"))]
const CONSOLE_LOG_FILTER: tracing_subscriber::filter::LevelFilter = {
	use tracing_subscriber::filter::LevelFilter;

	match cfg!(debug_assertions) {
		true => LevelFilter::DEBUG,
		false => LevelFilter::INFO,
	}
};

impl Node {
	pub async fn new(data_dir: impl AsRef<Path>) -> Result<(Arc<Node>, Arc<Router>), NodeError> {
		let data_dir = data_dir.as_ref().to_path_buf();
		#[cfg(debug_assertions)]
		let data_dir = data_dir.join("dev");

		// This error is ignored because it's throwing on mobile despite the folder existing.
		let _ = fs::create_dir_all(&data_dir).await;

		// let (non_blocking, _guard) = tracing_appender::non_blocking(rolling::daily(
		// 	Path::new(&data_dir).join("logs"),
		// 	"log",
		// ));
		// TODO: Make logs automatically delete after x time https://github.com/tokio-rs/tracing/pull/2169

		let subscriber = tracing_subscriber::registry().with(
			EnvFilter::from_default_env()
				.add_directive("warn".parse().expect("Error invalid tracing directive!"))
				.add_directive(
					"sd_core=debug"
						.parse()
						.expect("Error invalid tracing directive!"),
				)
				.add_directive(
					"sd_core::location::manager=info"
						.parse()
						.expect("Error invalid tracing directive!"),
				)
				.add_directive(
					"sd_core_mobile=debug"
						.parse()
						.expect("Error invalid tracing directive!"),
				)
				.add_directive(
					"server=debug"
						.parse()
						.expect("Error invalid tracing directive!"),
				)
				.add_directive(
					"desktop=debug"
						.parse()
						.expect("Error invalid tracing directive!"),
				), // .add_directive(
			    // 	"rspc=debug"
			    // 		.parse()
			    // 		.expect("Error invalid tracing directive!"),
			    // ),
		);
		#[cfg(not(feature = "android"))]
		let subscriber = subscriber.with(tracing_subscriber::fmt::layer().with_filter(CONSOLE_LOG_FILTER));
		#[cfg(feature = "android")]
		let subscriber = subscriber.with(tracing_android::layer("com.spacedrive.app").unwrap()); // TODO: This is not working
		subscriber
			// .with(
			// 	Layer::default()
			// 		.with_writer(non_blocking)
			// 		.with_ansi(false)
			// 		.with_filter(LevelFilter::DEBUG),
			// )
			.init();

		let event_bus = broadcast::channel(1024);
		let (reboot_tx, reboot_rx) = mpsc::channel(1);
		let config = Arc::new(NodeConfigManager::new(&data_dir).await?);

		let jobs = JobManager::new();
		let location_manager = LocationManager::new();
		let library_manager = LibraryManager::new(
			data_dir.join("libraries"),
			NodeContext {
				config: Arc::clone(&config),
				jobs: Arc::clone(&jobs),
				location_manager: Arc::clone(&location_manager),
				event_bus_tx: event_bus.0.clone(),
			},
		)
		.await?;

		Self::setup_location_management(&library_manager, &location_manager).await;

		// Trying to resume possible paused jobs
		let inner_library_manager = Arc::clone(&library_manager);
		let inner_jobs = Arc::clone(&jobs);
		tokio::spawn(async move {
			for library_ctx in inner_library_manager.get_all_libraries_ctx().await {
				if let Err(e) = Arc::clone(&inner_jobs).resume_jobs(&library_ctx).await {
					error!("Failed to resume jobs for library. {:#?}", e);
				}
			}
		});
		let jobs = Arc::new(RwLock::new(jobs));
		let library_manager = Arc::new(RwLock::new(library_manager));

		tokio::spawn(Self::handle_reboot(
			reboot_rx,
			Arc::clone(&library_manager),
			Arc::clone(&jobs),
			Arc::clone(&config),
			event_bus.0.clone(),
		));

		let router = api::mount();
		let node = Node {
			config,
			library_manager,
			jobs,
			event_bus,
			reboot_tx,
		};

		info!("Spacedrive online.");
		Ok((Arc::new(node), router))
	}

	pub fn get_request_context(&self) -> Ctx {
		Ctx {
			library_manager: Arc::clone(&self.library_manager),
			config: Arc::clone(&self.config),
			jobs: Arc::clone(&self.jobs),
			event_bus: self.event_bus.0.clone(),
			reboot_tx: self.reboot_tx.clone(),
		}
	}

	// Note: this system doesn't use chunked encoding which could prove a problem with large files but I can't see an easy way to do chunked encoding with Tauri custom URIs.
	pub async fn handle_custom_uri(
		&self,
		path: Vec<&str>,
	) -> (
		u16,     /* Status Code */
		&str,    /* Content-Type */
		Vec<u8>, /* Body */
	) {
		match path.first() {
			Some(&"thumbnail") => {
				if path.len() != 2 {
					return (
						400,
						"text/html",
						b"Bad Request: Invalid number of parameters".to_vec(),
					);
				}

				fs::read(
					self.config
						.data_directory()
						.join("thumbnails")
						.join(path[1] /* file_cas_id */)
						.with_extension("webp"),
				)
				.await
				.map(|buf| (200, "image/webp", buf))
				.unwrap_or_else(|e| {
					error!("Failed to read thumbnail file. {e}");

					(404, "text/html", b"File Not Found".to_vec())
				})
			}
			_ => (
				400,
				"text/html",
				b"Bad Request: Invalid operation!".to_vec(),
			),
		}
	}

	pub async fn shutdown(&self) {
		info!("Spacedrive shutting down...");
		self.jobs.read().await.pause().await;
		info!("Spacedrive Core shutdown successful!");
	}

	async fn setup_location_management(
		library_manager: &LibraryManager,
		location_manager: &LocationManager,
	) {
		// Adding already existing locations for location management
		for library_ctx in library_manager.get_all_libraries_ctx().await {
			for location in library_ctx
				.db
				.location()
				.find_many(vec![])
				.exec()
				.await
				.unwrap_or_else(|e| {
					error!("Failed to get locations from database for location manager: {e}");
					vec![]
				}) {
				if let Err(e) = location_manager.add(location.id, library_ctx.clone()).await {
					error!("Failed to add location to location manager: {e}");
				}
			}
		}
	}

	async fn handle_reboot(
		mut reboot_rx: mpsc::Receiver<NodeReboot>,
		library_manager: Arc<RwLock<Arc<LibraryManager>>>,
		jobs: Arc<RwLock<Arc<JobManager>>>,
		config: Arc<NodeConfigManager>,
		event_bus_tx: broadcast::Sender<CoreEvent>,
	) {
		async fn inner(
			force_reboot: bool,
			library_manager: Arc<RwLock<Arc<LibraryManager>>>,
			jobs: Arc<RwLock<Arc<JobManager>>>,
			config: Arc<NodeConfigManager>,
			event_bus_tx: broadcast::Sender<CoreEvent>,
		) -> Result<(), NodeError> {
			if force_reboot {
				let new_job_manager = JobManager::new();

				let location_manager = LocationManager::new();

				let new_library_manager = LibraryManager::new(
					config.data_directory(),
					NodeContext {
						config: Arc::clone(&config),
						jobs: Arc::clone(&new_job_manager),
						location_manager: Arc::clone(&location_manager),
						event_bus_tx: event_bus_tx.clone(),
					},
				)
				.await?;

				Node::setup_location_management(&new_library_manager, &location_manager).await;

				*library_manager.write().await = new_library_manager;
				*jobs.write().await = new_job_manager;
			} else {
				todo!("Graceful reboot");
			}

			Ok(())
		}

		while let Some(reboot) = reboot_rx.recv().await {
			if reboot
				.done_tx
				.send(
					inner(
						reboot.force,
						Arc::clone(&library_manager),
						Arc::clone(&jobs),
						Arc::clone(&config),
						event_bus_tx.clone(),
					)
					.await,
				)
				.is_err()
			{
				error!("Failed to send reboot done signal");
			}
		}
	}
}

/// Error type for Node related errors.
#[derive(Error, Debug)]
pub enum NodeError {
	#[error("Failed to create data directory: {0}")]
	FailedToCreateDataDirectory(#[from] io::Error),
	#[error("Failed to initialize config: {0}")]
	FailedToInitializeConfig(#[from] node::NodeConfigError),
	#[error("Failed to initialize library manager: {0}")]
	FailedToInitializeLibraryManager(#[from] library::LibraryManagerError),
	#[error("Location manager error: {0}")]
	LocationManager(#[from] LocationManagerError),
	#[error("Failed to reboot: {0}")]
	Reboot(String),
}
