import { VariantProps, cva } from 'class-variance-authority';
import clsx from 'clsx';
import { ComponentProps, forwardRef } from 'react';

import { FormControlProps, FormField, useFormControl } from './FormControl';

const inputStyles = cva(
	[
		'px-3 py-1 text-sm rounded-md border leading-7',
		'outline-none shadow-sm focus:ring-2 transition-all'
	],
	{
		variants: {
			variant: {
				default: [
					'bg-app-input focus:bg-app-focus placeholder-ink-faint border-app-line',
					'focus:ring-app-selected/30 focus:border-app-divider/80'
				]
			},
			size: {
				sm: 'text-sm',
				md: 'text-base'
			}
		},
		defaultVariants: {
			variant: 'default'
		}
	}
);

type Props = VariantProps<typeof inputStyles> & ComponentProps<'input'> & FormControlProps;

export const Input = forwardRef<HTMLInputElement, Props>(
	({ size, variant, className, ...props }, ref) => {
		const [controlProps, fieldProps] = useFormControl(props);

		const isControlled = 'ref' in props && 'name' in props && 'onChange' in props; // Is field controlled by react-hook-form

		if (!isControlled) {
			const onChange = fieldProps.onChange;
			fieldProps.onChange = (e) => onChange(e.target.value); // Use the form events target value.
		}

		return (
			<FormField {...controlProps}>
				<input
					ref={ref}
					className={clsx(inputStyles({ size, variant }), props.className)}
					{...fieldProps}
				/>
			</FormField>
		);
	}
);

// export const TextArea = ({
// 	size,
// 	variant,
// 	...props
// }: InputBaseProps & React.TextareaHTMLAttributes<HTMLTextAreaElement>) => {
// 	return <textarea {...props} className={clsx(inputStyles({ size, variant }), props.className)} />;
// };
// export function Label(props: PropsWithChildren<{ slug?: string }>) {
// 	return (
// 		<label className="text-sm font-bold" htmlFor={props.slug}>
// 			{props.children}
// 		</label>
// 	);
// }
