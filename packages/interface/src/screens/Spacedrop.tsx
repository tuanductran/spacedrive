// @ts-nocheck // TODO: Reenable this before merging PR
import ball from '@sd/assets/images/spacedrive_logo.png';
import { Laptop } from 'phosphor-react';
import { useEffect, useRef, useState } from 'react';
import { proxy } from 'valtio';

// TODO: Do an icon of the user if not fall back to a photo which represent the type of their device -> Phone or laptop

// TODO: Hook up backend
// - Subscribe to devices - emit all cached on startup and emit add/remove events
// - Emit send event onto a specific device with the content of the file
const placeholderData = [
	{
		id: 1,
		name: "Oscar's iPhone"
	},
	{
		id: 2,
		name: "Oscar's Fridge"
	},
	{
		id: 4,
		name: "Oscar's Laptop"
	}
];

// TODO: Move this React in `useState` in the future.
const state = proxy({
	isLoading: false
});

function doCanvasAnimation(c: HTMLCanvasElement, animate?: boolean) {
	const ctx = c.getContext('2d')!;
	let x0, y0, w, h, dw;

	function init() {
		w = window.innerWidth;
		h = window.innerHeight;
		c.width = w;
		c.height = h;
		let offset = h > 380 ? 100 : 65;
		offset = h > 800 ? 60 : offset;
		x0 = w / 2;
		y0 = h - offset;
		dw = Math.max(w, h, 1000) / 13;
		drawCircles();
	}
	window.onresize = init;

	function drawCircle(radius) {
		ctx.beginPath();
		const color = Math.round(255 * (1 - radius / Math.max(w, h)));
		ctx.strokeStyle = 'rgba(' + color + ',' + color + ',' + color + ',0.1)';
		ctx.arc(x0, y0, radius, 0, 2 * Math.PI);
		ctx.stroke();
		ctx.lineWidth = 2;
	}

	let step = 0;

	function drawCircles() {
		ctx.clearRect(0, 0, w, h);
		for (let i = 0; i < 8; i++) {
			drawCircle(dw * i + (step % dw));
		}
		step += 1;
	}

	function doAnimate() {
		if (state.isLoading || step % dw < dw - 5) {
			requestAnimationFrame(function () {
				drawCircles();
				doAnimate();
			});
		}
	}

	init();
	if (animate) {
		doAnimate();
	} else {
		drawCircles();
	}
}

export default function Spacedrop() {
	const canvasRef = useRef<HTMLCanvasElement>(null);

	useEffect(() => doCanvasAnimation(canvasRef.current!), []);

	useEffect(() => {
		let timeout;

		addEventListener('dragenter', () => {
			if (!state.isLoading) {
				state.isLoading = true;
				doCanvasAnimation(canvasRef.current!, true);
			}
		});

		addEventListener('dragleave', () => {
			if (state.isLoading) {
				state.isLoading = false;
				doCanvasAnimation(canvasRef.current!);
			}
		});

		addEventListener('dragover', (e) => {
			e.preventDefault();
		});

		addEventListener('drop', (e) => {
			console.log('DROP');
			e.preventDefault();

			if (state.isLoading) {
				state.isLoading = false;
				doCanvasAnimation(canvasRef.current!);
			}

			if (e.dataTransfer.items) {
				// Use DataTransferItemList interface to access the file(s)
				[...e.dataTransfer.items].forEach((item, i) => {
					// If dropped items aren't files, reject them
					if (item.kind === 'file') {
						const file = item.getAsFile();
						console.log(`… file[${i}].name = ${file.name}`);
					}
				});
			} else {
				// Use DataTransfer interface to access the file(s)
				[...e.dataTransfer.files].forEach((file, i) => {
					console.log(`… file[${i}].name = ${file.name}`);
				});
			}
		});

		return () => {
			// TODO: Remove event listeners
		};
	}, []);

	return (
		<>
			<canvas ref={canvasRef} className="absolute w-full h-full" />
			<div className="h-full w-full p-4 flex flex-col">
				<div className="flex-1 flex justify-center items-center">
					<div className="w-full h-60 flex space-x-8 items-center justify-center">
						{placeholderData.map((device) => (
							<div
								key={device.id}
								className="w-32 h-32 p-4 flex-col flex justify-center items-center text-center rounded-full bg-white bg-opacity-10 border border-white border-opacity-10"
							>
								<span>
									<Laptop size={46} className="" />
								</span>

								<h1>{device.name}</h1>
							</div>
						))}
					</div>
				</div>

				<div className="pb-0 p-8 relative bottom-0">
					<div className="flex justify-center items-center">
						<img src={ball} alt="ball" draggable={false} className="h-32 animate-sdpulse" />
					</div>
				</div>
			</div>
		</>
	);
}
