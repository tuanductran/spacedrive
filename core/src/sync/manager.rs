use crate::prisma::*;
use sd_sync::*;
use serde_json::{from_value, json, to_vec, Value};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::mpsc::{self, Receiver, Sender};
use uhlc::{HLCBuilder, HLC, NTP64};
use uuid::Uuid;

use super::ModelSyncData;

pub struct SyncManager {
	db: Arc<PrismaClient>,
	node: Uuid,
	_clocks: HashMap<Uuid, NTP64>,
	clock: HLC,
	tx: Sender<CRDTOperation>,
}

impl SyncManager {
	pub fn new(db: &Arc<PrismaClient>, node: Uuid) -> (Self, Receiver<CRDTOperation>) {
		let (tx, rx) = mpsc::channel(64);

		(
			Self {
				db: db.clone(),
				node,
				clock: HLCBuilder::new().with_id(node.into()).build(),
				_clocks: Default::default(),
				tx,
			},
			rx,
		)
	}

	pub async fn write_ops<'item, I: prisma_client_rust::BatchItem<'item>>(
		&self,
		tx: &PrismaClient,
		(ops, queries): (Vec<CRDTOperation>, I),
	) -> prisma_client_rust::Result<<I as prisma_client_rust::BatchItemParent>::ReturnValue> {
		let owned = ops
			.iter()
			.filter_map(|op| match &op.typ {
				CRDTOperationType::Owned(owned_op) => Some(tx.owned_operation().create(
					op.id.as_bytes().to_vec(),
					op.timestamp.0 as i64,
					to_vec(&owned_op.items).unwrap(),
					owned_op.model.clone(),
					node::pub_id::equals(op.node.as_bytes().to_vec()),
					vec![],
				)),
				_ => None,
			})
			.collect::<Vec<_>>();

		let shared = ops
			.iter()
			.filter_map(|op| match &op.typ {
				CRDTOperationType::Shared(shared_op) => {
					let kind = match &shared_op.data {
						SharedOperationData::Create(_) => "c",
						SharedOperationData::Update { .. } => "u",
						SharedOperationData::Delete => "d",
					};

					Some(tx.shared_operation().create(
						op.id.as_bytes().to_vec(),
						op.timestamp.0 as i64,
						shared_op.model.to_string(),
						to_vec(&shared_op.record_id).unwrap(),
						kind.to_string(),
						to_vec(&shared_op.data).unwrap(),
						node::pub_id::equals(op.node.as_bytes().to_vec()),
						vec![],
					))
				}
				_ => None,
			})
			.collect::<Vec<_>>();

		let (res, _) = tx._batch((queries, (owned, shared))).await?;

		for op in ops {
			self.tx.send(op).await.ok();
		}

		Ok(res)
	}

	pub async fn write_op<'item, Q: prisma_client_rust::BatchItem<'item>>(
		&self,
		tx: &PrismaClient,
		op: CRDTOperation,
		query: Q,
	) -> prisma_client_rust::Result<<Q as prisma_client_rust::BatchItemParent>::ReturnValue> {
		let ret = match &op.typ {
			CRDTOperationType::Owned(owned_op) => {
				tx._batch((
					tx.owned_operation().create(
						op.id.as_bytes().to_vec(),
						op.timestamp.0 as i64,
						to_vec(&owned_op.items).unwrap(),
						owned_op.model.clone(),
						node::pub_id::equals(op.node.as_bytes().to_vec()),
						vec![],
					),
					query,
				))
				.await?
				.1
			}
			CRDTOperationType::Shared(shared_op) => {
				let kind = match &shared_op.data {
					SharedOperationData::Create(_) => "c",
					SharedOperationData::Update { .. } => "u",
					SharedOperationData::Delete => "d",
				};

				tx._batch((
					tx.shared_operation().create(
						op.id.as_bytes().to_vec(),
						op.timestamp.0 as i64,
						shared_op.model.to_string(),
						to_vec(&shared_op.record_id).unwrap(),
						kind.to_string(),
						to_vec(&shared_op.data).unwrap(),
						node::pub_id::equals(op.node.as_bytes().to_vec()),
						vec![],
					),
					query,
				))
				.await?
				.1
			}
			_ => todo!(),
		};

		self.tx.send(op).await.ok();

		Ok(ret)
	}

	pub async fn get_ops(&self) -> prisma_client_rust::Result<Vec<CRDTOperation>> {
		Ok(self
			.db
			.shared_operation()
			.find_many(vec![])
			.order_by(shared_operation::timestamp::order(
				prisma_client_rust::Direction::Asc,
			))
			.include(shared_operation::include!({ node: select {
                pub_id
            } }))
			.exec()
			.await?
			.into_iter()
			.flat_map(|op| {
				Some(CRDTOperation {
					id: Uuid::from_slice(&op.id).ok()?,
					node: Uuid::from_slice(&op.node.pub_id).ok()?,
					timestamp: NTP64(op.timestamp as u64),
					typ: CRDTOperationType::Shared(SharedOperation {
						record_id: serde_json::from_slice(&op.record_id).ok()?,
						model: op.model,
						data: serde_json::from_slice(&op.data).ok()?,
					}),
				})
			})
			.collect())
	}

	pub async fn ingest_op(&self, op: CRDTOperation) -> prisma_client_rust::Result<()> {
		let db = &self.db;

		println!("ingesting op");

		// temporary
		self.db
			.node()
			.upsert(
				node::pub_id::equals(op.node.as_bytes().to_vec()),
				node::create_unchecked(
					op.node.as_bytes().to_vec(),
					"PLACEHOLDER".to_string(),
					vec![],
				),
				vec![],
			)
			.exec()
			.await?;

		match ModelSyncData::from_op(op.typ).unwrap() {
			ModelSyncData::FilePath(id, shared_op) => {
				let location = db
					.location()
					.find_unique(location::pub_id::equals(id.location.pub_id))
					.select(location::select!({ id }))
					.exec()
					.await?
					.unwrap();

				match shared_op {
					SharedOperationData::Create(SharedOperationCreateData::Unique(mut data)) => {
						db.file_path()
							.create(
								id.id,
								location::id::equals(location.id),
								serde_json::from_value(data.remove("materialized_path").unwrap())
									.unwrap(),
								serde_json::from_value(data.remove("name").unwrap()).unwrap(),
								serde_json::from_value(
									data.remove("extension").unwrap_or_else(|| {
										serde_json::Value::String("".to_string())
									}),
								)
								.unwrap(),
								data.into_iter()
									.flat_map(|(k, v)| file_path::SetParam::deserialize(&k, v))
									.collect(),
							)
							.exec()
							.await?;
					}
					SharedOperationData::Update { field, value } => {
						self.db
							.file_path()
							.update(
								file_path::location_id_id(location.id, id.id),
								vec![file_path::SetParam::deserialize(&field, value).unwrap()],
							)
							.exec()
							.await?;
					}
					_ => todo!(),
				}
			}
			ModelSyncData::Location(id, shared_op) => match shared_op {
				SharedOperationData::Create(SharedOperationCreateData::Unique(mut data)) => {
					db.location()
						.create(
							id.pub_id,
							serde_json::from_value(data.remove("name").unwrap()).unwrap(),
							serde_json::from_value(data.remove("path").unwrap()).unwrap(),
							{
								let val: std::collections::HashMap<String, Value> =
									from_value(data.remove("node").unwrap()).unwrap();
								let val = val.into_iter().next().unwrap();

								node::UniqueWhereParam::deserialize(&val.0, val.1).unwrap()
							},
							data.into_iter()
								.flat_map(|(k, v)| location::SetParam::deserialize(&k, v))
								.collect(),
						)
						.exec()
						.await?;
				}
				_ => todo!(),
			},
			ModelSyncData::Object(id, shared_op) => match shared_op {
				SharedOperationData::Create(_) => {
					db.object()
						.upsert(
							object::pub_id::equals(id.pub_id.clone()),
							(id.pub_id, vec![]),
							vec![],
						)
						.exec()
						.await
						.ok();
				}
				SharedOperationData::Update { field, value } => {
					db.object()
						.update(
							object::pub_id::equals(id.pub_id),
							vec![object::SetParam::deserialize(&field, value).unwrap()],
						)
						.exec()
						.await?;
				}
				_ => todo!(),
			},
			ModelSyncData::Tag(id, shared_op) => match shared_op {
				SharedOperationData::Create(create_data) => match create_data {
					SharedOperationCreateData::Unique(create_data) => {
						db.tag()
							.create(
								id.pub_id,
								create_data
									.into_iter()
									.flat_map(|(field, value)| {
										tag::SetParam::deserialize(&field, value)
									})
									.collect(),
							)
							.exec()
							.await?;
					}
					_ => unreachable!(),
				},
				SharedOperationData::Update { field, value } => {
					db.tag()
						.update(
							tag::pub_id::equals(id.pub_id),
							vec![tag::SetParam::deserialize(&field, value).unwrap()],
						)
						.exec()
						.await?;
				}
				SharedOperationData::Delete => {
					db.tag()
						.delete(tag::pub_id::equals(id.pub_id))
						.exec()
						.await?;
				}
			},
			_ => todo!(),
		}

		Ok(())
	}

	fn new_op(&self, typ: CRDTOperationType) -> CRDTOperation {
		let timestamp = self.clock.new_timestamp();

		CRDTOperation {
			node: self.node,
			timestamp: *timestamp.get_time(),
			id: Uuid::new_v4(),
			typ,
		}
	}

	pub fn owned_create<
		const SIZE: usize,
		TSyncId: SyncId<ModelTypes = TModel>,
		TModel: SyncType<Marker = OwnedSyncType>,
	>(
		&self,
		id: TSyncId,
		values: [(&'static str, Value); SIZE],
	) -> CRDTOperation {
		self.new_op(CRDTOperationType::Owned(OwnedOperation {
			model: TModel::MODEL.to_string(),
			items: [(id, values)]
				.into_iter()
				.map(|(id, data)| OwnedOperationItem {
					id: json!(id),
					data: OwnedOperationData::Create(
						data.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
					),
				})
				.collect(),
		}))
	}
	pub fn owned_create_many<
		const SIZE: usize,
		TSyncId: SyncId<ModelTypes = TModel>,
		TModel: SyncType<Marker = OwnedSyncType>,
	>(
		&self,
		data: impl IntoIterator<Item = (TSyncId, [(&'static str, Value); SIZE])>,
		skip_duplicates: bool,
	) -> CRDTOperation {
		self.new_op(CRDTOperationType::Owned(OwnedOperation {
			model: TModel::MODEL.to_string(),
			items: vec![OwnedOperationItem {
				id: Value::Null,
				data: OwnedOperationData::CreateMany {
					values: data
						.into_iter()
						.map(|(id, data)| {
							(
								json!(id),
								data.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
							)
						})
						.collect(),
					skip_duplicates,
				},
			}],
		}))
	}
	pub fn owned_update<
		TSyncId: SyncId<ModelTypes = TModel>,
		TModel: SyncType<Marker = OwnedSyncType>,
	>(
		&self,
		id: TSyncId,
		values: impl IntoIterator<Item = (&'static str, Value)>,
	) -> CRDTOperation {
		self.new_op(CRDTOperationType::Owned(OwnedOperation {
			model: TModel::MODEL.to_string(),
			items: [(id, values)]
				.into_iter()
				.map(|(id, data)| OwnedOperationItem {
					id: json!(id),
					data: OwnedOperationData::Update(
						data.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
					),
				})
				.collect(),
		}))
	}

	pub fn shared_create<
		TSyncId: SyncId<ModelTypes = TModel>,
		TModel: SyncType<Marker = SharedSyncType>,
	>(
		&self,
		id: TSyncId,
	) -> CRDTOperation {
		self.new_op(CRDTOperationType::Shared(SharedOperation {
			model: TModel::MODEL.to_string(),
			record_id: json!(id),
			data: SharedOperationData::Create(SharedOperationCreateData::Atomic),
		}))
	}
	pub fn unique_shared_create<
		const SIZE: usize,
		TSyncId: SyncId<ModelTypes = TModel>,
		TModel: SyncType<Marker = SharedSyncType>,
	>(
		&self,
		id: TSyncId,
		values: [(&'static str, Value); SIZE],
	) -> CRDTOperation {
		self.new_op(CRDTOperationType::Shared(SharedOperation {
			model: TModel::MODEL.to_string(),
			record_id: json!(id),
			data: SharedOperationData::Create(SharedOperationCreateData::Unique(
				values
					.into_iter()
					.map(|(name, value)| (name.to_string(), value))
					.collect(),
			)),
		}))
	}
	pub fn shared_update<
		TSyncId: SyncId<ModelTypes = TModel>,
		TModel: SyncType<Marker = SharedSyncType>,
	>(
		&self,
		id: TSyncId,
		field: &str,
		value: Value,
	) -> CRDTOperation {
		self.new_op(CRDTOperationType::Shared(SharedOperation {
			model: TModel::MODEL.to_string(),
			record_id: json!(id),
			data: SharedOperationData::Update {
				field: field.to_string(),
				value,
			},
		}))
	}

	// fn compare_messages(&self, operations: Vec<CRDTOperation>) -> Vec<(CRDTOperation, bool)> {
	// 	operations
	// 		.into_iter()
	// 		.map(|op| (op.id, op))
	// 		.collect::<HashMap<_, _>>()
	// 		.into_iter()
	// 		.filter_map(|(_, op)| {
	// 			match &op.typ {
	// 				CRDTOperationType::Owned(_) => {
	// 					self._operations.iter().find(|find_op| match &find_op.typ {
	// 						CRDTOperationType::Owned(_) => {
	// 							find_op.timestamp >= op.timestamp && find_op.node == op.node
	// 						}
	// 						_ => false,
	// 					})
	// 				}
	// 				CRDTOperationType::Shared(shared_op) => {
	// 					self._operations.iter().find(|find_op| match &find_op.typ {
	// 						CRDTOperationType::Shared(find_shared_op) => {
	// 							shared_op.model == find_shared_op.model
	// 								&& shared_op.record_id == find_shared_op.record_id
	// 								&& find_op.timestamp >= op.timestamp
	// 						}
	// 						_ => false,
	// 					})
	// 				}
	// 				CRDTOperationType::Relation(relation_op) => {
	// 					self._operations.iter().find(|find_op| match &find_op.typ {
	// 						CRDTOperationType::Relation(find_relation_op) => {
	// 							relation_op.relation == find_relation_op.relation
	// 								&& relation_op.relation_item == find_relation_op.relation_item
	// 								&& relation_op.relation_group == find_relation_op.relation_group
	// 						}
	// 						_ => false,
	// 					})
	// 				}
	// 			}
	// 			.map(|old_op| (old_op.timestamp != op.timestamp).then_some(true))
	// 			.unwrap_or(Some(false))
	// 			.map(|old| (op, old))
	// 		})
	// 		.collect()
	// }

	// pub fn receive_crdt_operations(&mut self, ops: Vec<CRDTOperation>) {
	// 	for op in &ops {
	// 		self._clock
	// 			.update_with_timestamp(&Timestamp::new(op.timestamp, op.node.into()))
	// 			.ok();

	// 		self._clocks.insert(op.node, op.timestamp);
	// 	}

	// 	for (op, old) in self.compare_messages(ops) {
	// 		let push_op = op.clone();

	// 		if !old {
	// 			match op.typ {
	// 				CRDTOperationType::Shared(shared_op) => match shared_op.model.as_str() {
	// 					"Object" => {
	// 						let id = shared_op.record_id;

	// 						match shared_op.data {
	// 							SharedOperationData::Create(SharedOperationCreateData::Atomic) => {
	// 								self.objects.insert(
	// 									id,
	// 									Object {
	// 										id,
	// 										..Default::default()
	// 									},
	// 								);
	// 							}
	// 							SharedOperationData::Update { field, value } => {
	// 								let mut file = self.objects.get_mut(&id).unwrap();

	// 								match field.as_str() {
	// 									"name" => {
	// 										file.name = from_value(value).unwrap();
	// 									}
	// 									_ => unreachable!(),
	// 								}
	// 							}
	// 							SharedOperationData::Delete => {
	// 								self.objects.remove(&id).unwrap();
	// 							}
	// 							_ => {}
	// 						}
	// 					}
	// 					_ => unreachable!(),
	// 				},
	// 				CRDTOperationType::Owned(owned_op) => match owned_op.model.as_str() {
	// 					"FilePath" => {
	// 						for item in owned_op.items {
	// 							let id = from_value(item.id).unwrap();

	// 							match item.data {
	// 								OwnedOperationData::Create(data) => {
	// 									self.file_paths.insert(
	// 										id,
	// 										from_value(Value::Object(data.into_iter().collect()))
	// 											.unwrap(),
	// 									);
	// 								}
	// 								OwnedOperationData::Update(data) => {
	// 									let obj = self.file_paths.get_mut(&id).unwrap();

	// 									for (key, value) in data {
	// 										match key.as_str() {
	// 											"path" => obj.path = from_value(value).unwrap(),
	// 											"file" => obj.file = from_value(value).unwrap(),
	// 											_ => unreachable!(),
	// 										}
	// 									}
	// 								}
	// 								OwnedOperationData::Delete => {
	// 									self.file_paths.remove(&id);
	// 								}
	// 							}
	// 						}
	// 					}
	// 					_ => unreachable!(),
	// 				},
	// 				CRDTOperationType::Relation(relation_op) => match relation_op.relation.as_str()
	// 				{
	// 					"TagOnObject" => match relation_op.data {
	// 						RelationOperationData::Create => {
	// 							self.tags_on_objects.insert(
	// 								(relation_op.relation_item, relation_op.relation_group),
	// 								TagOnObject {
	// 									object_id: relation_op.relation_item,
	// 									tag_id: relation_op.relation_group,
	// 								},
	// 							);
	// 						}
	// 						RelationOperationData::Update { field: _, value: _ } => {
	// 							// match field.as_str() {
	// 							// 	_ => unreachable!(),
	// 							// }
	// 						}
	// 						RelationOperationData::Delete => {
	// 							self.tags_on_objects
	// 								.remove(&(
	// 									relation_op.relation_item,
	// 									relation_op.relation_group,
	// 								))
	// 								.unwrap();
	// 						}
	// 					},
	// 					_ => unreachable!(),
	// 				},
	// 			}

	// 			self._operations.push(push_op)
	// 		}
	// 	}
	// }
}
