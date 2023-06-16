import { Heart } from 'phosphor-react-native';
import { useState } from 'react';
import { Pressable, PressableProps } from 'react-native';
import { Object as SDObject, useLibraryMutation } from '@sd/client';

type Props = {
	data: SDObject;
	style: PressableProps['style'];
};

const FavoriteButton = (props: Props) => {
	const [favorite, setFavorite] = useState(props.data.favorite);

	const updateObject = useLibraryMutation('objects.update', {
		onSuccess: () => {
			// TODO: Invalidate search queries
			setFavorite(!favorite);
		}
	});

	return (
		<Pressable
			disabled={updateObject.isLoading}
			onPress={() => updateObject.mutate([props.data.id, { favorite: !favorite }])}
			style={props.style}
		>
			<Heart color="white" size={22} weight={favorite ? 'fill' : 'regular'} />
		</Pressable>
	);
};

export default FavoriteButton;
