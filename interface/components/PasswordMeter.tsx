import clsx from 'clsx';
import { usePasswordStrength } from '@sd/client';

export interface PasswordMeterProps {
	password: string;
}

export const PasswordMeter = (props: PasswordMeterProps) => {
	const result = usePasswordStrength(props.password);
	if (!result) return null; // We are lazy loading `zxcvbn`. Given we eager load, this *should* rarely be hit.
	const { score, scoreText } = result;

	return (
		<div className="relative">
			<h3 className="text-sm">Password strength</h3>
			<span
				className={clsx(
					'absolute right-0 top-0.5 px-1 text-sm font-semibold',
					score === 0 && 'text-red-500',
					score === 1 && 'text-red-500',
					score === 2 && 'text-amber-400',
					score === 3 && 'text-lime-500',
					score === 4 && 'text-accent'
				)}
			>
				{scoreText}
			</span>
			<div className="flex grow">
				<div className="mt-2 w-full rounded-full bg-app-box/50">
					<div
						style={{
							width: `${score !== 0 ? score * 25 : 12.5}%`
						}}
						className={clsx(
							'h-2 rounded-full transition-all',
							score === 0 && 'bg-red-500',
							score === 1 && 'bg-red-500',
							score === 2 && 'bg-amber-400',
							score === 3 && 'bg-lime-500',
							score === 4 && 'bg-accent'
						)}
					/>
				</div>
			</div>
		</div>
	);
};
