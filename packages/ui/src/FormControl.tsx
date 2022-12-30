import clsx from 'clsx';
import { ReactNode, useId } from 'react';
import { FieldError, useFormContext } from 'react-hook-form';

export interface FormControlProps {
	id?: string;
	name?: string;
	label?: string;
	hideLabel?: boolean;
	children?: ReactNode;
}

export function useFormControl<T extends FormControlProps>(props: T) {
	const id = useId();
	const { label, hideLabel, ...otherProps } = props;

	return [
		{ id, name: props.name, label, hideLabel },
		{ id, ...otherProps }
	] as const;
}

export function useFieldError(name?: string): FieldError | null | undefined {
	if (!name) return null;

	const {
		formState: { errors }
	} = useFormContext();

	// @ts-expect-error: Incorrect types in recent releases: https://github.com/react-hook-form/react-hook-form/issues/8619
	return errors[name];
}

export function FormField({ id, label, hideLabel, name, children }: FormControlProps) {
	const error = useFieldError(name);

	return (
		<div role="group">
			<label
				htmlFor={id}
				className={clsx(hideLabel ? 'sr-only' : 'text-secondary text-sm font-medium')}
			>
				{label}
			</label>
			<div className={clsx(label && !hideLabel && 'mt-1')}>{children}</div>
			{error && <p className="mt-2 text-sm text-red-600 dark:text-red-400">{error.message}</p>}
		</div>
	);
}
