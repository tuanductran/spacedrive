/// This system allows connecting with [Redux DevTools Extension](https://github.com/zalmoxisus/redux-devtools-extension).
import type {} from '@redux-devtools/extension';

// FIXME https://github.com/reduxjs/redux-devtools/issues/1097
type Message = {
	type: string;
	payload?: any;
	state?: any;
};

declare global {
	interface Window {
		__NORMI_DEVTOOLS__?: {
			onChange(type: string | object, data: any): void;
		};
	}
}

export function devtoolsInit() {
	let extension: typeof window['__REDUX_DEVTOOLS_EXTENSION__'] | false;
	try {
		extension = window.__REDUX_DEVTOOLS_EXTENSION__;
	} catch {
		// ignored
	}

	if (!extension) {
		console.warn('[Warning] Please install/enable Redux devtools extension');
		return;
	}

	const devtools = extension.connect({ name: 'Normi Cache' });
	devtools.init({ updatedAt: new Date().toLocaleString() });

	window.__NORMI_DEVTOOLS__ = {
		onChange(type, data) {
			devtools.send(
				typeof type === 'string'
					? { type, updatedAt: new Date().toLocaleString() }
					: ({
							updatedAt: new Date().toLocaleString(),
							...type
					  } as any),
				data
			);
		}
	};
}
