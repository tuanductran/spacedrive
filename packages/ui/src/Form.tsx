import { ComponentProps } from 'react';
import { FieldValues, FormProvider, SubmitHandler, UseFormReturn } from 'react-hook-form';

interface FormProps<T extends FieldValues = any>
	extends Omit<ComponentProps<'form'>, 'onSubmit' | 'as'> {
	form: UseFormReturn<T>;
	onSubmit: SubmitHandler<T>;
}

export function Form<T extends FieldValues>({ form, onSubmit, children, ...props }: FormProps<T>) {
	// TODO: Allow default focusing a field by name on this?

	return (
		<FormProvider {...form}>
			<form onSubmit={form.handleSubmit(onSubmit)} {...props}>
				<fieldset disabled={form.formState.isSubmitting}>{children}</fieldset>
			</form>
		</FormProvider>
	);
}
