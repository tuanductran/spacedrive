use normi::{normi, Object};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::RouterBuilder;

#[derive(Serialize, Type, Object)]
#[normi(rename = "org")]
pub struct Organisation {
	#[normi(id)]
	pub id: String,
	pub name: String,
	#[normi(refr)]
	pub users: Vec<User>,
	#[normi(refr)]
	pub owner: User,
	pub non_normalised_data: Vec<()>,
}

#[derive(Debug, Deserialize, Serialize, Type, Object)]
pub struct User {
	#[normi(id)]
	pub id: String,
	pub name: String,
}

#[derive(Serialize, Type, Object)]
pub struct CompositeId {
	#[normi(id)]
	pub org_id: String,
	#[normi(id)]
	pub user_id: String,
}

pub fn mount() -> RouterBuilder {
	RouterBuilder::new()
		.query("user", |t| {
			t.resolver(|_, _: ()| async move {
				Ok(User {
					id: "1".to_string(),
					name: "Monty Beaumont".to_string(),
				})
			})
			.map(normi)
		})
		.query("org", |t| {
			t.resolver(|_, _: ()| async move {
				Ok(Organisation {
					id: "org-1".into(),
					name: "Org 1".into(),
					users: vec![
						User {
							id: "user-1".into(),
							name: "Monty Beaumont".into(),
						},
						User {
							id: "user-2".into(),
							name: "Millie Beaumont".into(),
						},
						User {
							id: "user-3".into(),
							name: "Oscar Beaumont".into(),
						},
					],
					owner: User {
						id: "user-1".into(),
						name: "Monty Beaumont".into(),
					},
					non_normalised_data: vec![(), ()],
				})
			})
			.map(normi)
		})
		.query("compositeKey", |t| {
			t.resolver(|_, _: ()| async move {
				Ok(CompositeId {
					org_id: "org-1".into(),
					user_id: "user-1".into(),
				})
			})
			.map(normi)
		})
		.mutation("updateUser", |t| {
			t.resolver(|ctx, user: User| async move {
				// ctx.invalidation_manager.invalidate(library_id, user);
				ctx.invalidation_manager.invalidate_global(user).await;
				Ok(())
			})
		})
}
