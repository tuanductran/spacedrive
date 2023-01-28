use crate::node::NodeConfig;

use rspc::{ErrorCode, Type};
use serde::{Deserialize, Serialize};

use super::{Ctx, RouterBuilder};

#[derive(Serialize, Deserialize, Debug, Type)]
struct NodeState {
	#[serde(flatten)]
	config: NodeConfig,
	data_path: String,
}

pub fn mount() -> RouterBuilder {
	RouterBuilder::new()
		.query("buildInfo", |t| {
			#[derive(Serialize, Type)]
			pub struct BuildInfo {
				version: &'static str,
				commit: &'static str,
			}

			t(|_, _: ()| BuildInfo {
				version: env!("CARGO_PKG_VERSION"),
				commit: env!("GIT_HASH"),
			})
		})
		.query("nodeState", |t| {
			t(|ctx: Ctx, _: ()| async move {
				Ok(NodeState {
					config: ctx.config.get().await,
					// We are taking the assumption here that this value is only used on the frontend for display purposes
					data_path: ctx
						.config
						.data_directory()
						.to_str()
						.expect("Found non-UTF-8 path")
						.to_string(),
				})
			})
		})
		.mutation("updateNodeConfig", |t| {
			t(|ctx: Ctx, new_config: NodeConfig| async move {
				ctx.config
					.write(|mut config| *config = new_config)
					.await
					.map_err(|e| {
						rspc::Error::with_cause(
							ErrorCode::InternalServerError,
							"Failed to update config".to_string(),
							e,
						)
					})
			})
		})
		.mutation("reboot", |t| {
			t(|ctx: Ctx, force: bool| async move {
				ctx.reboot(force).await.map_err(|e| {
					rspc::Error::with_cause(
						ErrorCode::InternalServerError,
						"Failed to reboot".to_string(),
						e,
					)
				})
			})
		})
}
