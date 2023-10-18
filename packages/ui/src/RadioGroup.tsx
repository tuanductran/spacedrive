'use client';

/* eslint-disable tailwindcss/migration-from-tailwind-2 */
import * as RadioGroup from '@radix-ui/react-radio-group';
import clsx from 'clsx';
import { forwardRef } from 'react';

export interface RootProps extends RadioGroup.RadioGroupProps {}
export const Root = forwardRef<HTMLDivElement, RootProps>(
	({ children, className, ...props }, ref) => {
		return (
			<RadioGroup.Root {...props} ref={ref}>
				<div className={clsx('space-y-3', className)}>{children}</div>
			</RadioGroup.Root>
		);
	}
);

// export const Item = tw(
// 	RadioGroup.Item
// )`rounded-md border border-app-line bg-app-box px-4 py-2 flex items-center space-x-2`;

export interface ItemProps extends RadioGroup.RadioGroupItemProps {}
export const Item = ({ children, ...props }: ItemProps) => {
	return (
		<div
			className={clsx(
				'flex max-w-sm space-x-2 rounded-md border border-app-line bg-app-box/50 px-4 py-3',
				props.disabled && 'opacity-30'
			)}
		>
			<RadioGroup.Item
				id={'radio' + props.value}
				className={clsx(
					'peer relative mr-1 mt-1 h-4 w-4 flex-shrink-0 rounded-full border border-app-line',
					'radix-state-checked:bg-accent',
					'radix-state-unchecked:bg-app-input',
					'focus:outline-none focus:ring-0 focus:ring-offset-0 focus-visible:ring focus-visible:ring-accent focus-visible:ring-opacity-75 focus-visible:ring-offset-2 dark:focus-visible:ring-offset-gray-800'
				)}
				{...props}
			>
				<RadioGroup.Indicator className="leading-0 absolute inset-0 flex items-center justify-center">
					<div className="h-1.5 w-1.5 rounded-full bg-white"></div>
				</RadioGroup.Indicator>
			</RadioGroup.Item>
			<label className="" htmlFor={'radio' + props.value}>
				{children}
			</label>
		</div>
	);
};
