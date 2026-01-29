export const logger = {
	info: (msg: string, meta?: object) => {
		console.log(
			JSON.stringify({
				level: "info",
				message: msg,
				...meta,
				timestamp: Date.now(),
			}),
		);
	},
	error: (msg: string, error?: unknown) => {
		console.error(
			JSON.stringify({
				level: "error",
				message: msg,
				error: String(error),
				timestamp: Date.now(),
			}),
		);
	},
	warn: (msg: string, meta?: object) => {
		console.warn(
			JSON.stringify({
				level: "warn",
				message: msg,
				...meta,
				timestamp: Date.now(),
			}),
		);
	},
};
