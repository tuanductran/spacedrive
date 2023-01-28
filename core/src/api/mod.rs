use crate::{
	job::JobManager,
	library::LibraryManager,
	node::{NodeConfigManager, NodeReboot},
	NodeError,
};

use std::sync::Arc;

use rspc::{Config, Type};
use serde::Serialize;
use tokio::sync::oneshot;
use tokio::{
	sync::{broadcast, mpsc, RwLock, RwLockReadGuard},
	time::{Duration, Instant},
};

mod core;
mod files;
mod jobs;
mod keys;
mod libraries;
mod locations;
mod normi;
mod tags;
pub mod utils;
pub mod volumes;

use utils::{InvalidRequests, InvalidateOperationEvent};

pub type Router = rspc::Router<Ctx>;
pub(crate) type RouterBuilder = rspc::RouterBuilder<Ctx>;

/// Represents an internal core event, these are exposed to client via a rspc subscription.
#[derive(Debug, Clone, Serialize, Type)]
pub enum CoreEvent {
	NewThumbnail { cas_id: String },
	InvalidateOperation(InvalidateOperationEvent),
	InvalidateOperationDebounced(InvalidateOperationEvent),
}

/// Is provided when executing the router from the request.
pub struct Ctx {
	pub library_manager: Arc<RwLock<Arc<LibraryManager>>>,
	pub config: Arc<NodeConfigManager>,
	pub jobs: Arc<RwLock<Arc<JobManager>>>,
	pub event_bus: broadcast::Sender<CoreEvent>,
	pub(super) reboot_tx: mpsc::Sender<NodeReboot>,
}

impl Ctx {
	async fn reboot(&self, force: bool) -> Result<(), NodeError> {
		let (done_tx, done_rx) = oneshot::channel();

		self.reboot_tx
			.send(NodeReboot { force, done_tx })
			.await
			.map_err(|_| NodeError::Reboot("Failed to send reboot request".to_string()))?;

		done_rx.await.unwrap_or_else(|_| {
			Err(NodeError::Reboot(
				"Failed to receive reboot response".to_string(),
			))
		})
	}

	async fn jobs(&self) -> RwLockReadGuard<Arc<JobManager>> {
		self.jobs.read().await
	}

	async fn library_manager(&self) -> RwLockReadGuard<Arc<LibraryManager>> {
		self.library_manager.read().await
	}
}

pub(crate) fn mount() -> Arc<Router> {
	let config = Config::new().set_ts_bindings_header("/* eslint-disable */");

	#[cfg(all(debug_assertions, not(feature = "mobile")))]
	let config = config.export_ts_bindings(
		std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../packages/client/src/core.ts"),
	);

	let r = <Router>::new()
		.config(config)
		.merge("core.", core::mount())
		.merge("normi.", normi::mount())
		.merge("library.", libraries::mount())
		.merge("volumes.", volumes::mount())
		.merge("tags.", tags::mount())
		.merge("keys.", keys::mount())
		.merge("locations.", locations::mount())
		.merge("files.", files::mount())
		.merge("jobs.", jobs::mount())
		// TODO: Scope the invalidate queries to a specific library (filtered server side)
		.subscription("invalidateQuery", |t| {
			t(|ctx: Ctx, _: ()| {
				let mut event_bus_rx = ctx.event_bus.subscribe();
				let mut last = Instant::now();
				async_stream::stream! {
					while let Ok(event) = event_bus_rx.recv().await {
						match event {
							CoreEvent::InvalidateOperation(op) => yield op,
							CoreEvent::InvalidateOperationDebounced(op) => {
								let current = Instant::now();
								if current.duration_since(last) > Duration::from_millis(1000 / 10) {
									last = current;
									yield op;
								}
							},
							_ => {}
						}
					}
				}
			})
		})
		.build()
		.arced();
	InvalidRequests::validate(Arc::clone(&r)); // This validates all invalidation calls.

	r
}

#[cfg(test)]
mod tests {
	/// This test will ensure the rspc router and all calls to `invalidate_query` are valid and also export an updated version of the Typescript bindings.
	#[test]
	fn test_and_export_rspc_bindings() {
		super::mount();
	}
}
