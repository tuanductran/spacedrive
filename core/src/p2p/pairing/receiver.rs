//! A receiver of the pairing process is the one that receives the pairing request from the initiator.

use sd_p2p::PeerId;
use uuid::Uuid;

/// Frontend event related to a pairing process for an receiver
pub enum ReceiverPairingEvent {
	Started { id: Uuid },
	PromptForPassword,
	// TODO
}

/// TODO
pub struct ReceiverPairingManager {
	to: PeerId,
}

impl ReceiverPairingManager {
	pub fn new() -> Self {
		todo!();
	}
}
