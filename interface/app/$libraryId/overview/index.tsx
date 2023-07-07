import { RefObject, memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import 'react-loading-skeleton/dist/skeleton.css';
import { Category, ExplorerItem } from '@sd/client';
import { useCallbackToWatchResize } from '~/hooks';
import { ExplorerContext } from '../Explorer/Context';
import ContextMenu from '../Explorer/ContextMenu';
// import ContextMenu from '../Explorer/FilePath/ContextMenu';
import { Inspector } from '../Explorer/Inspector';
import { DefaultTopBarOptions } from '../Explorer/TopBarOptions';
import View from '../Explorer/View';
import { useExplorerStore } from '../Explorer/store';
import { usePageLayoutContext } from '../PageLayout/Context';
import { TopBarPortal } from '../TopBar/Portal';
import Statistics from '../overview/Statistics';
import { Categories } from './Categories';
import { useItems } from './data';

const OverviewInspector = memo(
	(props: { categoriesRef: RefObject<HTMLDivElement>; selectedItem?: ExplorerItem }) => {
		const explorerStore = useExplorerStore();
		const { ref: pageRef } = usePageLayoutContext();

		const [height, setHeight] = useState(0);

		const updateHeight = useCallback(() => {
			if (props.categoriesRef.current && pageRef.current) {
				const categoriesBottom = props.categoriesRef.current.getBoundingClientRect().bottom;
				const pageBottom = pageRef.current.getBoundingClientRect().bottom;

				setHeight(Math.trunc(pageBottom - categoriesBottom));
			}
		}, [props.categoriesRef, pageRef]);

		useCallbackToWatchResize(updateHeight, [updateHeight], pageRef);

		useEffect(() => {
			const element = pageRef.current;
			if (!element) return;

			updateHeight();

			element.addEventListener('scroll', updateHeight);
			return () => element.removeEventListener('scroll', updateHeight);
		}, [pageRef, updateHeight]);

		if (!height) return null;

		return (
			<Inspector
				data={props.selectedItem}
				showThumbnail={explorerStore.layoutMode !== 'media'}
				className="no-scrollbar sticky top-[68px] w-[260px] shrink-0 bg-app pb-4 pl-1.5 pr-1"
				style={{ height }}
			/>
		);
	}
);

export const Component = () => {
	const explorerStore = useExplorerStore();
	const { ref: pageRef } = usePageLayoutContext();

	const categoriesRef = useRef<HTMLDivElement>(null);

	const [selectedCategory, setSelectedCategory] = useState<Category>('Recents');

	const { items, query, loadMore } = useItems(selectedCategory);

	const [selectedItemId, setSelectedItemId] = useState<number>();

	const selectedItem = useMemo(
		() => (selectedItemId ? items?.find((item) => item.item.id === selectedItemId) : undefined),
		[selectedItemId, items]
	);

	useEffect(() => {
		if (pageRef.current) {
			const { scrollTop } = pageRef.current;
			if (scrollTop > 100) pageRef.current.scrollTo({ top: 100 });
		}
	}, [selectedCategory, pageRef]);

	return (
		<ExplorerContext.Provider value={{}}>
			<TopBarPortal right={<DefaultTopBarOptions />} />

			<div>
				<Statistics />

				<div ref={categoriesRef} className="sticky top-0 z-10">
					<Categories
						selected={selectedCategory}
						onSelectedChanged={setSelectedCategory}
					/>
				</div>

				<div className="flex">
					<View
						items={query.isLoading ? null : items || []}
						scrollRef={pageRef}
						onLoadMore={loadMore}
						rowsBeforeLoadMore={5}
						selected={selectedItemId}
						onSelectedChange={setSelectedItemId}
						top={68}
						className={explorerStore.layoutMode === 'rows' ? 'min-w-0' : undefined}
						contextMenu={selectedItem ? <ContextMenu item={selectedItem} /> : null}
					/>

					{explorerStore.showInspector && (
						<OverviewInspector
							categoriesRef={categoriesRef}
							selectedItem={selectedItem}
						/>
					)}
				</div>
			</div>
		</ExplorerContext.Provider>
	);
};
