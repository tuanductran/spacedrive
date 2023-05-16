import { useState } from 'react';
import { PeerMetadata, useBridgeSubscription } from '~/../packages/client/src';
import { Button } from '~/../packages/ui/src/Button';
import { Heading } from '../Layout';

export const Component = () => {
	return (
		<>
			<Heading
				title="Nodes"
				description="Manage the nodes connected to this library. A node is an instance of Spacedrive's backend, running on a device or server. Each node carries a copy of the database and synchronizes via peer-to-peer connections in realtime."
			/>
			<TempPairingProcess />
		</>
	);
};

// A temporary UI for pairing a node. This can be redesigned into a modal in the future.
function TempPairingProcess() {
	const [[discoveredPeers], setDiscoveredPeer] = useState([new Map<string, PeerMetadata>()]);
	// const doSpacedrop = useBridgeMutation('p2p.spacedrop');

	useBridgeSubscription(['p2p.events'], {
		onData(data) {
			if (data.type === 'DiscoveredPeer') {
				setDiscoveredPeer([discoveredPeers.set(data.peer_id, data.metadata)]);
			}
		}
	});

	return (
		<>
			{Object.entries(discoveredPeers).map(([peer_id, peer]) => (
				<div key={peer_id}>
					{peer.name} ({peer_id})
					<Button
						variant="accent"
						className="ml-4"
						onClick={() => {
							console.log('Pair with peer', peer_id);
						}}
					>
						Pair
					</Button>
				</div>
			))}
		</>
	);
}
