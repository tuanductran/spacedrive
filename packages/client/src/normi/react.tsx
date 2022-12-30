import { CustomHooks } from '@rspc/client';
import { useQuery as __useQuery, useQueryClient } from '@tanstack/react-query';
import { PropsWithChildren, createContext, useContext, useEffect, useMemo, useState } from 'react';

import { getLibraryIdRaw } from '../hooks';
import { useBridgeSubscription, useInvalidateQuery } from '../rspc';
import {
	NormiCache,
	_internal_createNormiHooks,
	_internal_queryFn,
	denormalise,
	getNormiCache,
	subscribeToKey
} from './utils';

const ctx = createContext<NormiCache>(undefined!);

export function NormiProvider({
	children,
	contextSharing
}: PropsWithChildren<{ contextSharing?: boolean }>) {
	const normiCache = getNormiCache(contextSharing ?? false);
	return <ctx.Provider value={normiCache}>{children}</ctx.Provider>;
}

export function useNormiCache(): NormiCache {
	const normiCache = useContext(ctx);
	if (!normiCache)
		throw new Error(
			'Normi cache not found. Ensure you have mounted the `<NormiProvider>` component higher in your component tree.'
		);
	return normiCache;
}

export function useNormi() {
	const queryClient = useQueryClient();
	const normiCache = useNormiCache();
	return _internal_createNormiHooks(queryClient, normiCache);
}

export function normiCustomHooks(
	opts: { contextSharing?: boolean },
	nextHooks?: () => CustomHooks
): () => CustomHooks {
	const next = nextHooks?.();
	return () => ({
		mapQueryKey: next?.mapQueryKey,
		doQuery: next?.doQuery,
		doMutation: next?.doMutation,
		dangerous: {
			useQuery(keyAndInput, handler, opts) {
				const normiCache = useNormiCache();
				const query = __useQuery(
					keyAndInput,
					_internal_queryFn(keyAndInput, handler, normiCache),
					opts
				);

				const [state, setState] = useState(0);

				useEffect(
					() => subscribeToKey(keyAndInput, () => setState((v) => v + 1), normiCache),
					[keyAndInput]
				);

				return useMemo(
					() => ({
						...query,
						data: denormalise(query.data, normiCache)
					}),
					[query, state]
				);
			}
		}
	});
}
