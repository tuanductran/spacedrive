import { ComponentProps, ReactNode, forwardRef } from 'react';

import { FormControlProps, FormField, useFormControl } from '../FormControl';
import { Input as RawInput } from '../Input';

type Props = ComponentProps<typeof RawInput> & FormControlProps;

export const Input = forwardRef<HTMLInputElement, Props>((props, ref) => {
	const [controlProps, fieldProps] = useFormControl(props);

	const isControlled = 'ref' in props && 'name' in props && 'onChange' in props;

	if (!isControlled) {
		const onChange = fieldProps.onChange;
		fieldProps.onChange = (e) => onChange(e.target.value);
	}

	return (
		<FormField {...controlProps}>
			<RawInput ref={ref} {...fieldProps} />
		</FormField>
	);
});
