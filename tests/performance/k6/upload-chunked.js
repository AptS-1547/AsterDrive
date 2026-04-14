import { sleep } from "k6";
import { Trend } from "k6/metrics";

import { benchConfig, durationEnv, intEnv } from "./lib/config.js";
import {
	completeUpload,
	ensureRootFolder,
	initChunkedUpload,
	login,
	maybeRefreshSession,
	uniqueName,
	uploadChunk,
} from "./lib/client.js";
import { createSummary } from "./lib/summary.js";

const flowDuration = new Trend("aster_upload_chunked_flow_duration", true);
const totalBytes = intEnv("ASTER_BENCH_CHUNKED_TOTAL_BYTES", 10 * 1024 * 1024);
let state;

export const options = {
	vus: intEnv("ASTER_BENCH_CHUNKED_UPLOAD_VUS", 3),
	duration: durationEnv("ASTER_BENCH_CHUNKED_UPLOAD_DURATION", "30s"),
	thresholds: {
		http_req_failed: ["rate<0.01"],
		aster_upload_chunked_flow_duration: [
			`p(95)<${intEnv("ASTER_BENCH_CHUNKED_UPLOAD_P95_MS", 4000)}`,
		],
	},
};

function makeChunk(length, ordinal) {
	const seed = String.fromCharCode(65 + (ordinal % 26));
	return seed.repeat(length);
}

export function setup() {
	const session = login();
	const folderId = ensureRootFolder(session, benchConfig.chunkedUploadFolder);
	return {
		session,
		folderId,
	};
}

export default function (data) {
	if (!state) {
		state = data;
	}

	state.session = maybeRefreshSession(state.session);
	const startedAt = Date.now();
	const { body } = initChunkedUpload(state.session, {
		filename: uniqueName("chunked-upload", "bin"),
		totalSize: totalBytes,
		folderId: state.folderId,
	});
	const sessionData = body.data;
	if (sessionData.mode !== "chunked") {
		throw new Error(
			`expected chunked upload mode, got ${sessionData.mode}; increase ASTER_BENCH_CHUNKED_TOTAL_BYTES`,
		);
	}

	for (let index = 0; index < sessionData.total_chunks; index += 1) {
		const remaining = totalBytes - sessionData.chunk_size * index;
		const chunkSize =
			index === sessionData.total_chunks - 1
				? remaining
				: sessionData.chunk_size;
		uploadChunk(
			state.session,
			sessionData.upload_id,
			index,
			makeChunk(chunkSize, index),
		);
	}
	completeUpload(state.session, sessionData.upload_id);
	flowDuration.add(Date.now() - startedAt);

	if (benchConfig.thinkTimeMs > 0) {
		sleep(benchConfig.thinkTimeMs / 1000);
	}
}

export const handleSummary = createSummary("upload-chunked", [
	"aster_upload_chunked_flow_duration",
]);

