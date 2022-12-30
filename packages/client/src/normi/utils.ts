import { QueryCache, QueryClient } from '@tanstack/react-query';

import { isEqual } from './isEqual';

export type NormiCache = {
	// The normalised data
	cache: Map<string /* `$ty-$id */, unknown>;
	// Stores which entities are referenced by which queries
	dependencies: Map<string /* `$ty-$id */, Set<string /* queryKey */>>;
	// Allows us to cause a query to be refreshed from the normalised cache
	subscriptions: Map<string /* queryKey */, Set<() => void>>;
};

declare global {
	interface Window {
		normiCache?: NormiCache;
	}
}

// getNormiCache is a function that returns a new NormiCache or the global one based on if contextSharing is enabled.
export function getNormiCache(contextSharing: boolean): NormiCache {
	if (contextSharing) {
		if (window.normiCache === undefined) {
			window.normiCache = {
				cache: new Map(),
				dependencies: new Map(),
				subscriptions: new Map()
			};
		}

		return window.normiCache;
	} else {
		return {
			cache: new Map(),
			dependencies: new Map(),
			subscriptions: new Map()
		};
	}
}

// Determine the normalised cache key for a given type and id.
export function cacheKey($ty: string, $id: any) {
	if ($id === undefined || $id === null) {
		throw new Error('Normi: Error creating cacheKey with empty $id');
	}
	if (Array.isArray($id) || typeof $id === 'object') return `${$ty}-${JSON.stringify($id)}`;
	return `${$ty}-${$id}`;
}

export function subscriptionKey(keyAndInput: [string] | [string, anys]) {
	return JSON.stringify(keyAndInput);
}

// Takes the data from it's normalised form and replaces references with the actual data from the local cache.
export function denormalise(value: any, normiCache: NormiCache): any {
	if (typeof value === 'undefined' || value === null) {
		return value;
	} else if (Array.isArray(value)) {
		return value.map((v) => denormalise(v, normiCache));
	} else if (typeof value === 'object') {
		if ('$ty' in value && '$id' in value) {
			const key = cacheKey(value.$ty, value.$id);
			const result = normiCache.cache.get(key);
			if (result === undefined)
				console.warn(`Normi: Couldn't find key '${key}' in cache but it was used in operation.`);
			return result;
		}

		const newValue = Object.create(value);
		for (const [k, v] of Object.entries(value)) {
			newValue[k] = denormalise(v, normiCache);
		}
		return newValue;
	}

	return value;
}

// Scan a type and it's sub fields for references to other types and update them into the global cache.
export function scanAndInsertIntoNormiCache(
	value: any,
	changedKeys: Set<string>,
	normiCache: NormiCache
): any {
	if (typeof value === 'undefined' || value === null) {
		return value;
	} else if (Array.isArray(value)) {
		value.map((v) => scanAndInsertIntoNormiCache(v, changedKeys, normiCache));
	} else if (typeof value === 'object') {
		if ('$ty' in value && '$id' in value) {
			const key = cacheKey(value.$ty, value.$id);
			delete value.$ty;
			delete value.$id;
			const existingValue: any = normiCache.cache.get(key);
			if (!isEqual(existingValue, value)) changedKeys.add(key);
			normiCache.cache.set(key, { ...existingValue, ...value });
		}

		for (const [k, v] of Object.entries(value)) {
			scanAndInsertIntoNormiCache(v, changedKeys, normiCache);
		}
	}

	return value;
}

// Create a new subscription to a given cache key.
export function subscribeToKey(
	keyAndInput: [string] | [string, any],
	callback: () => void,
	normiCache: NormiCache
): () => void {
	const key = subscriptionKey(keyAndInput);
	const set = normiCache.subscriptions.get(key) || new Set();
	set.add(callback);
	normiCache.subscriptions.set(key, set);
	return () => normiCache.subscriptions.get(key)?.delete(callback);
}

// _internal_createHooks is a function that creates the normi hooks agnostic of the frontend framework.
export function _internal_createNormiHooks(queryClient: QueryClient, normiCache: NormiCache) {
	return {
		mutate(obj: any, opts?: { _reason?: string }) {
			const changedKeys = new Set<string>();
			scanAndInsertIntoNormiCache(obj, changedKeys, normiCache);
			if (window.__NORMI_DEVTOOLS__ && changedKeys.size > 0)
				window.__NORMI_DEVTOOLS__.onChange(
					opts?._reason ?? 'mutate',
					Object.fromEntries(normiCache.cache)
				);

			const invalidatedQueries = new Set<string>();
			changedKeys.forEach((key) => {
				normiCache.dependencies.get(key)?.forEach((queryKey) => {
					if (invalidatedQueries.has(queryKey)) return;
					invalidatedQueries.add(queryKey);

					normiCache.subscriptions.get(queryKey)?.forEach((callback) => callback());
				});
			});
		}
		// clear() {
		// 	queryClient.clear();
		// 	normiCache.cache.clear();
		// 	normiCache.dependencies.clear();
		// 	normiCache.subscriptions.clear();
		// }
	};
}

export function _internal_queryFn(
	keyAndInput: [string] | [string, any],
	queryFn: (keyAndInput: [string] | [string, any]) => Promise<any>,
	normiCache: NormiCache
): () => Promise<any> {
	const queryKey = JSON.stringify(keyAndInput);

	return async () => {
		const data = await queryFn(keyAndInput);
		let initialQueryCache: Map<string, unknown> = undefined!;
		if (window.__NORMI_DEVTOOLS__) initialQueryCache = new Map(normiCache.cache);

		if (typeof data === 'object' && '$refs' in data) {
			data.$refs.forEach((v: any) => {
				const key = cacheKey(v.$ty, v.$id);
				delete v.$ty;
				delete v.$id;
				normiCache.cache.set(key, v);

				const deps = normiCache.dependencies.get(key) || new Set();
				deps.add(queryKey);
				normiCache.dependencies.set(key, deps);
			});
		}

		if (window.__NORMI_DEVTOOLS__) {
			const data = Object.fromEntries(normiCache.cache);
			if (!isEqual(Object.fromEntries(initialQueryCache), data))
				window.__NORMI_DEVTOOLS__.onChange(
					{
						type: 'query response',
						queryKey: keyAndInput
					},
					Object.fromEntries(normiCache.cache)
				);
		}

		return typeof data === 'object' && '$data' in data ? data.$data : data;
	};
}
