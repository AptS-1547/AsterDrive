import { runtimeFlags } from "@/config/runtime";

const PREFIX = "[AsterDrive]";

function warn(msg: string, ...args: unknown[]) {
	console.warn(PREFIX, msg, ...args);
}

function error(msg: string, ...args: unknown[]) {
	console.error(PREFIX, msg, ...args);
}

function debug(msg: string, ...args: unknown[]) {
	if (runtimeFlags.isDev) {
		console.debug(PREFIX, msg, ...args);
	}
}

export const logger = { warn, error, debug };
