import { useCallback, useState } from 'react';
import { Text, View } from 'react-native';
import { useDebouncedCallback } from 'use-debounce';
import { Object as SDObject, useLibraryMutation } from '@sd/client';

type Props = {
	data: SDObject;
};

const Note = (props: Props) => {
	const [note, setNote] = useState(props.data.note || '');

	const updateObject = useLibraryMutation('objects.update');

	const debounce = useDebouncedCallback(
		(note: string) => updateObject.mutate([props.data.id, { note }]),
		2000
	);

	const debouncedNote = useCallback((note: string) => debounce(note), [debounce]);

	return (
		<View>
			<Text>Note</Text>
		</View>
	);
};

export default Note;
