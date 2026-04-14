import { sleep } from "k6";
import { Trend } from "k6/metrics";

import { benchConfig, durationEnv, intEnv } from "./lib/config.js";
import {
	downloadFile,
	findFileInFolder,
	login,
	maybeRefreshSession,
	resolveRootFolderId,
} from "./lib/client.js";
import { createSummary } from "./lib/summary.js";

const downloadDuration = new Trend("aster_download_duration", true);
let state;

export const options = {
	vus: intEnv("ASTER_BENCH_DOWNLOAD_VUS", 6),
	duration: durationEnv("ASTER_BENCH_DOWNLOAD_DURATION", "30s"),
	thresholds: {
		http_req_failed: ["rate<0.01"],
		aster_download_duration: [
			`p(95)<${intEnv("ASTER_BENCH_DOWNLOAD_P95_MS", 1000)}`,
		],
	},
};

export function setup() {
	const session = login();
	const folderId = resolveRootFolderId(session, benchConfig.downloadFolder);
	if (!folderId) {
		throw new Error(
			`missing seeded folder ${benchConfig.downloadFolder}; run bun tests/performance/seed.mjs first`,
		);
	}

	const fileId = findFileInFolder(session, folderId, benchConfig.downloadFile);
	if (!fileId) {
		throw new Error(
			`missing seeded file ${benchConfig.downloadFile}; run bun tests/performance/seed.mjs first`,
		);
	}

	return {
		session,
		fileId,
	};
}

export default function (data) {
	if (!state) {
		state = data;
	}

	state.session = maybeRefreshSession(state.session);
	const response = downloadFile(state.session, state.fileId);
	downloadDuration.add(response.timings.duration);

	if (benchConfig.thinkTimeMs > 0) {
		sleep(benchConfig.thinkTimeMs / 1000);
	}
}

export const handleSummary = createSummary("download", [
	"aster_download_duration",
]);

