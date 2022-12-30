use std::{collections::VecDeque, sync::Arc, time::Duration};

use futures::TryFutureExt;
use normi::{normalise, Object};
use rspc::{internal::specta::DataType, RouterBuilder};
use serde_json::{Map, Value};
use tokio::{
	sync::{
		broadcast::{self, error::RecvError},
		mpsc,
	},
	time::sleep,
};
use tracing::warn;
use uuid::Uuid;

use crate::api::Router;

#[cfg(debug_assertions)]
use std::sync::Mutex;

/// holds information about all invalidation queries done with the [`invalidate_query!`] macro so we can check they are valid when building the router.
#[cfg(debug_assertions)]
pub(crate) static INVALIDATION_REQUESTS: Mutex<InvalidRequests> =
	Mutex::new(InvalidRequests::new());

/// a request to invalidate a specific resource
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct InvalidationRequest {
	pub key: &'static str,
	pub arg_ty: Option<DataType>,
	pub macro_src: &'static str,
}

/// invalidation request for a specific resource
#[derive(Debug, Default)]
#[allow(dead_code)]
pub(crate) struct InvalidRequests {
	pub queries: Vec<InvalidationRequest>,
}

impl InvalidRequests {
	#[allow(unused)]
	const fn new() -> Self {
		Self {
			queries: Vec::new(),
		}
	}

	#[allow(unused_variables)]
	pub(crate) fn validate(r: Arc<Router>) {
		#[cfg(debug_assertions)]
		{
			let invalidate_requests = INVALIDATION_REQUESTS.lock().unwrap();

			let queries = r.queries();
			for req in &invalidate_requests.queries {
				if let Some(query_ty) = queries.get(req.key) {
					if let Some(arg) = &req.arg_ty {
						if &query_ty.ty.input != arg {
							panic!(
								"Error at '{}': Attempted to invalid query '{}' but the argument type does not match the type defined on the router.",
								req.macro_src, req.key
                        	);
						}
					}
				} else {
					panic!(
						"Error at '{}': Attempted to invalid query '{}' which was not found in the router",
						req.macro_src, req.key
					);
				}
			}
		}
	}
}

/// `invalidate_query` is a macro which stores a list of all of it's invocations so it can ensure all of the queries match the queries attached to the router.
/// This allows invalidate to be type-safe even when the router keys are stringly typed.
/// ```ignore
/// invalidate_query!(
/// library, // crate::library::LibraryContext
/// "version": (), // Name of the query and the type of it
/// () // The arguments
/// );
/// ```
#[macro_export]
#[allow(clippy::crate_in_macro_def)]
macro_rules! invalidate_query {
	($ctx:expr, $key:literal) => {{
		let ctx: &crate::library::LibraryContext = &$ctx; // Assert the context is the correct type

		#[cfg(debug_assertions)]
		{
			#[ctor::ctor]
			fn invalidate() {
				crate::api::utils::INVALIDATION_REQUESTS
					.lock()
					.unwrap()
					.queries
					.push(crate::api::utils::InvalidationRequest {
						key: $key,
						arg_ty: None,
            			macro_src: concat!(file!(), ":", line!()),
					})
			}
		}

		// The error are ignored here because they aren't mission critical. If they fail the UI might be outdated for a bit.
		ctx.node_context.invalidation_manager.unchecked_invalidate_key(Some(ctx.id), $key, None);
	}};
	($ctx:expr, $key:literal: $arg_ty:ty, $arg:expr $(,)?) => {{
		let _: $arg_ty = $arg; // Assert the type the user provided is correct
		let ctx: &crate::library::LibraryContext = &$ctx; // Assert the context is the correct type

		#[cfg(debug_assertions)]
		{
			#[ctor::ctor]
			fn invalidate() {
				crate::api::utils::INVALIDATION_REQUESTS
					.lock()
					.unwrap()
					.queries
					.push(crate::api::utils::InvalidationRequest {
						key: $key,
						arg_ty: Some(<$arg_ty as rspc::internal::specta::Type>::reference(rspc::internal::specta::DefOpts {
                            parent_inline: false,
                            type_map: &mut rspc::internal::specta::TypeDefs::new(),
                        }, &[])),
                        macro_src: concat!(file!(), ":", line!()),
					})
			}
		}

		// The error are ignored here because they aren't mission critical. If they fail the UI might be outdated for a bit.
		let _ = serde_json::to_value($arg)
			.map(|v|
				ctx.node_context.invalidation_manager.unchecked_invalidate_key(Some(ctx.id), $key, Some(v));
			)
			.map_err(|_| {
				tracing::warn!("Invalidate query error: Failed to serialize query arguments!");
			});
	}};
}

pub struct InvalidationManager(mpsc::Sender<(Option<Uuid /* library_id */>, Value)>);

impl InvalidationManager {
	pub fn new<TCtx: Send + Sync + 'static>() -> (RouterBuilder<TCtx>, Arc<Self>) {
		let (tx, mut rx) = mpsc::channel::<(Option<Uuid /* library_id */>, Value)>(30);
		let (tx2, rx2) = broadcast::channel(30);

		// We queue all messages to the frontend and push them every 200 milliseconds
		tokio::spawn(async move {
			let mut queued = VecDeque::with_capacity(15);

			loop {
				tokio::select! {
					_ = sleep(Duration::from_millis(200)) => {
						let len = queued.len();
						// If an item was removed from the queue between getting the length and draining this could panic. I don't think this will be an issue given how we are using it.
						let queue = (&mut queued).drain(0..len).collect::<Vec<_>>();

						if queue.len() > 0 {
							println!("DRAIN {:?}", queue); // TODO: Remove
							let _ = tx2.send(queue).map_err(|_| warn!("Error sending invalidate event on broadcast channel."));
						}
					},
					event = (&mut rx).recv() => {
						if let Some(event) = event {
							println!("QUEUEING {:?}", event); // TODO: Remove
							(&mut queued).push_back(event);
						} else {
							warn!("Shutting down the invalidation handler thread due to `InvalidationManager` being dropped.");
							return;
						}
					}
				}
			}
		});

		let router = rspc::Router::<TCtx>::new().subscription("invalidate", move |t| {
			t(move |_, library_id: Uuid| {
				let mut rx2 = rx2.resubscribe();
				async_stream::stream! {
					loop {
						match rx2.recv().await {
							Ok(values) => yield values.into_iter().filter_map(|(lid, value)| (lid.is_none() || lid == Some(library_id)).then(|| value)).collect::<Vec<_>>(),
							Err(RecvError::Lagged(i)) => {
								warn!("'invalidQuery' subscription reported lag, missing '{}' messages on the broadcast channel.", i);
								return;
							},
							Err(RecvError::Closed) => return,
						}
					}
				}
			})
		});

		(router, Arc::new(Self(tx)))
	}

	/// Invalidate an rspc operation on the frontend. This method does no check the key is a valid one so you should use the `invalidate_query!` macro instead.
	pub fn unchecked_invalidate_key(
		&self,
		library_id: Option<Uuid>,
		query_key: &'static str,
		args: Option<Value>,
	) {
		let _ = self
			.0
			.send((
				library_id,
				Value::Object({
					let mut obj = Map::new();
					obj.insert(
						"$invalidate".into(),
						Value::Array(match args {
							Some(args) => vec![query_key.into(), args],
							None => vec![query_key.into()],
						}),
					);
					obj
				}),
			))
			.map_err(|_| warn!("Error sending invalidate event on mpsc channel."));
	}

	/// invalidate a normalized object in a library.
	/// WARNING: This invalidates all normalised objects but not the query itself.
	pub async fn invalidate(&self, library_id: Uuid, value: impl Object) {
		let value = normalise(value).unwrap();

		let _ = self
			.0
			.send((
				Some(library_id),
				match value {
					Value::Object(mut obj) => {
						let _ = obj.remove("$data");
						Value::Object(obj)
					}
					_ => unreachable!(),
				},
			))
			.await
			.map_err(|_| warn!("Error sending invalidate event on mpsc channel."));
	}

	pub async fn invalidate_global(&self, value: impl Object) {
		let value = normalise(value).unwrap();

		println!("GLOBAL {:?}", value); // TODO: Remove

		let _ = self
			.0
			.send((
				None,
				match value {
					Value::Object(mut obj) => {
						let _ = obj.remove("$data");
						Value::Object(obj)
					}
					_ => unreachable!(),
				},
			))
			.await
			.unwrap();
		// .map_err(|_| warn!("Error sending invalidate event on mpsc channel."));
	}
}
