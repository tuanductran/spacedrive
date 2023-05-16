//! The initiator of a pairing process is the one that starts the process.

use sd_p2p::PeerId;
use uuid::Uuid;

/// Frontend event related to a pairing process for an initiator
pub enum InitiatorPairingEvent {
	Started { id: Uuid },
	ShowPassword { password: String },
	// TODO
}

/// TODO
pub struct InitiatorPairingManager {
	to: PeerId,
}

impl InitiatorPairingManager {
	pub fn new() -> Self {
		todo!();
	}
}
