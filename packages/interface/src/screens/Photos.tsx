import { useBridgeMutation, useBridgeQuery, useNormi } from '@sd/client';
import { Button } from '@sd/ui';

export default function PhotosScreen() {
	return (
		<div className="flex flex-col w-full h-screen p-5 custom-scroll page-scroll app-background">
			<div className="flex flex-col space-y-5 pb-7">
				<p className="px-5 py-3 mb-3 text-sm border rounded-md shadow-sm border-app-line bg-app-box ">
					<b>Note: </b>This is a pre-alpha build of Spacedrive, many features are yet to be
					functional.
				</p>
				{/* <Spline
					style={{ height: 500 }}
					height={500}
					className="rounded-md shadow-sm pointer-events-auto"
					scene="https://prod.spline.design/KUmO4nOh8IizEiCx/scene.splinecode"
				/> */}
				<Debug />
			</div>
		</div>
	);
}

// TODO: Remove this
function Debug() {
	const normi = useNormi();
	const query = useBridgeQuery(['normi.org']);
	const query2 = useBridgeQuery(['normi.user']);
	const query3 = useBridgeQuery(['normi.compositeKey']);
	const mutation = useBridgeMutation(['normi.updateUser']);

	console.log('RENDER DEBUG');

	return (
		<>
			<h1>Normalised cache debugger</h1>
			<p>Org: {JSON.stringify(query.data, undefined, 2)}</p>
			<p>User: {JSON.stringify(query2.data)}</p>
			<p>Composite Key: {JSON.stringify(query3.data)}</p>
			<Button
				variant="gray"
				onClick={() => {
					normi.mutate([
						{
							$ty: 'org',
							$id: 'org-1',
							name: 'New name'
						}
					]);
				}}
			>
				Local Mutation
			</Button>
			<Button
				variant="gray"
				onClick={() => {
					mutation.mutate({
						id: 'a',
						name: 'The server set this name!'
					});
				}}
			>
				Server Mutation
			</Button>
		</>
	);
}
