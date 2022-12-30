import * as SelectPrimitive from '@radix-ui/react-select';
import { ReactComponent as ChevronDouble } from '@sd/assets/svgs/chevron-double.svg';
import clsx from 'clsx';
import { CaretDown, Check } from 'phosphor-react';
import { ComponentProps, forwardRef } from 'react';
import {
	Controller,
	FieldPath,
	FieldValues,
	UseControllerProps,
	useController,
	useForm
} from 'react-hook-form';

import { FormControlProps, FormField, useFormControl } from '../FormControl';
import { Select as RawSelect } from '../Select';

// TODO: Cleanup this type
// export interface SelectProps extends ComponentProps<'select'>, FormControlProps {}

export interface Props2<T extends FieldValues, TName extends FieldPath<T>>
	extends UseControllerProps<T, TName> {
	// name: keyof T;
	// control: UseControllerProps<T>['control'];
}

function SelectInner<T extends FieldValues, TName extends FieldPath<T>>(props: Props2<T, TName>) {
	return null;
}

// export const Select = forwardRef<HTMLSelectElement, UseControllerProps & Omit<SelectProps, 'name'>>(
// 	(props, ref) => {
// 		const [controlProps, fieldProps] = useFormControl(props);

// 		// TODO: Break into separate component to respect conditional hook rules
// 		if ('control' in props) {
// 			// const {  } = fieldProps; // TODO: Split out controller and field props
// 			const {
// 				field: { value, onChange }
// 			} = useController(props);

// 			return (
// 				<FormField {...controlProps}>
// 					<RawSelect
// 						// ref={ref}
// 						value={value}
// 						onChange={(value) => {
// 							onChange(value);
// 							// props.onChange?.(value);
// 						}}
// 						children={props.children}
// 					/>
// 				</FormField>
// 			);
// 		}

// 		return (
// 			<FormField {...controlProps}>
// 				<RawSelect
// 					// ref={ref}
// 					value={props.value}
// 					onChange={props.onChange}
// 					children={props.children}
// 				/>
// 			</FormField>
// 		);
// 	}
// );

// export const ControlledSelect = () => {};

// TODO: Remove this
const demo = () => {
	const form = useForm({
		defaultValues: {
			abc: '123'
		}
	});
	return <SelectInner name="abc" control={form.control}></SelectInner>;
};
