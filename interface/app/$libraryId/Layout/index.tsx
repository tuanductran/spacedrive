import clsx from 'clsx';
import { Suspense, useEffect, useMemo, useRef } from 'react';
import { Navigate, Outlet } from 'react-router-dom';
import {
	ClientContextProvider,
	initPlausible,
	LibraryContextProvider,
	useBridgeQuery,
	useClientContext,
	usePlausibleEvent,
	usePlausiblePageViewMonitor,
	usePlausiblePingMonitor
} from '@sd/client';
import { useRootContext } from '~/app/RootContext';
import { LibraryIdParamsSchema } from '~/app/route-schemas';
import {
	useKeybindEventHandler,
	useOperatingSystem,
	useShowControls,
	useZodRouteParams
} from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { QuickPreviewContextProvider } from '../Explorer/QuickPreview/Context';
import { LayoutContext } from './Context';
import Sidebar from './Sidebar';

const Layout = () => {
	const { libraries, library } = useClientContext();
	const os = useOperatingSystem();
	useKeybindEventHandler(library?.uuid);

	const transparentBg = useShowControls().transparentBg;
	const plausibleEvent = usePlausibleEvent();
	const buildInfo = useBridgeQuery(['buildInfo']);

	const layoutRef = useRef<HTMLDivElement>(null);

	initPlausible({
		platformType: usePlatform().platform === 'tauri' ? 'desktop' : 'web',
		buildInfo: buildInfo?.data
	});

	const { rawPath } = useRootContext();

	usePlausiblePageViewMonitor({ currentPath: rawPath });
	usePlausiblePingMonitor({ currentPath: rawPath });

	useEffect(() => {
		const interval = setInterval(() => {
			plausibleEvent({
				event: {
					type: 'ping'
				}
			});
		}, 270 * 1000);

		return () => clearInterval(interval);
	}, []);

	const ctxValue = useMemo(() => ({ ref: layoutRef }), [layoutRef]);

	if (library === null && libraries.data) {
		const firstLibrary = libraries.data[0];

		if (firstLibrary) return <Navigate to={`/${firstLibrary.uuid}/overview`} replace />;
		else return <Navigate to="/" replace />;
	}

	return (
		<LayoutContext.Provider value={ctxValue}>
			<div
				ref={layoutRef}
				className={clsx(
					// App level styles
					'flex h-screen cursor-default select-none overflow-hidden text-ink',
					os === 'macOS' && 'has-blur-effects rounded-[10px]',
					os !== 'browser' && os !== 'windows' && 'frame border border-transparent'
				)}
				onContextMenu={(e) => {
					// TODO: allow this on some UI text at least / disable default browser context menu
					e.preventDefault();
					return false;
				}}
			>
				<Sidebar />
				<div
					className={clsx(
						'relative flex w-full overflow-hidden',
						transparentBg ? 'bg-app/80' : 'bg-app'
					)}
				>
					{library ? (
						<QuickPreviewContextProvider>
							<LibraryContextProvider library={library}>
								<Suspense fallback={<div className="h-screen w-screen bg-app" />}>
									<Outlet />
								</Suspense>
							</LibraryContextProvider>
						</QuickPreviewContextProvider>
					) : (
						<h1 className="p-4 text-white">
							Please select or create a library in the sidebar.
						</h1>
					)}
				</div>
			</div>
		</LayoutContext.Provider>
	);
};

export const Component = () => {
	const { libraryId } = useZodRouteParams(LibraryIdParamsSchema);

	return (
		<ClientContextProvider currentLibraryId={libraryId ?? null}>
			<Layout />
		</ClientContextProvider>
	);
};
