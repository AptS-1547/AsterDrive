const DEFAULT_LIST_SIZES = "100,1000,10000";
const DEFAULT_DOWNLOAD_BYTES = 5 * 1024 * 1024;
const DEFAULT_UPLOAD_CONCURRENCY = 16;
const LIST_FOLDER_PREFIX = process.env.ASTER_BENCH_LIST_FOLDER_PREFIX || "bench-list";
const DOWNLOAD_FOLDER = process.env.ASTER_BENCH_DOWNLOAD_FOLDER || "bench-download";
const DOWNLOAD_FILE = process.env.ASTER_BENCH_DOWNLOAD_FILE || "payload-5mb.bin";
const BATCH_TARGET_FOLDER =
	process.env.ASTER_BENCH_BATCH_TARGET_FOLDER || "bench-batch-target";
const WEBDAV_ROOT_FOLDER = process.env.ASTER_BENCH_WEBDAV_ROOT_FOLDER || "bench-webdav";

const config = {
	baseUrl: stripTrailingSlash(
		process.env.ASTER_BENCH_BASE_URL || "http://127.0.0.1:3000",
	),
	username: process.env.ASTER_BENCH_USERNAME || "bench_user",
	password: process.env.ASTER_BENCH_PASSWORD || "bench-pass-1234",
	email: process.env.ASTER_BENCH_EMAIL || "bench_user@example.com",
	searchTerm: process.env.ASTER_BENCH_SEARCH_TERM || "needle",
	webdavUsername: process.env.ASTER_BENCH_WEBDAV_USERNAME || "bench_webdav",
	webdavPassword:
		process.env.ASTER_BENCH_WEBDAV_PASSWORD || "bench_webdav_pass123",
	listSizes: parseListSizes(
		process.env.ASTER_BENCH_LIST_SIZES || DEFAULT_LIST_SIZES,
	),
	downloadBytes: parseIntEnv(
		process.env.ASTER_BENCH_DOWNLOAD_BYTES,
		DEFAULT_DOWNLOAD_BYTES,
	),
	uploadConcurrency: parseIntEnv(
		process.env.ASTER_BENCH_SEED_UPLOAD_CONCURRENCY,
		DEFAULT_UPLOAD_CONCURRENCY,
	),
};

function stripTrailingSlash(value) {
	return value.endsWith("/") ? value.slice(0, -1) : value;
}

function parseIntEnv(rawValue, fallback) {
	if (!rawValue) {
		return fallback;
	}

	const parsed = Number.parseInt(rawValue, 10);
	if (Number.isNaN(parsed) || parsed <= 0) {
		throw new Error(`invalid integer value: ${rawValue}`);
	}

	return parsed;
}

function parseListSizes(rawValue) {
	return rawValue
		.split(",")
		.map((item) => Number.parseInt(item.trim(), 10))
		.filter((item) => Number.isFinite(item) && item > 0);
}

function encodeQuery(params) {
	const query = new URLSearchParams();
	for (const [key, value] of Object.entries(params)) {
		if (value === undefined || value === null || value === "") {
			continue;
		}
		query.set(key, String(value));
	}

	const raw = query.toString();
	return raw ? `?${raw}` : "";
}

async function readJson(response) {
	const text = await response.text();
	let body;
	try {
		body = text ? JSON.parse(text) : null;
	} catch {
		body = text;
	}

	if (!response.ok) {
		throw new Error(
			`${response.status} ${response.statusText}: ${
				typeof body === "string" ? body : JSON.stringify(body)
			}`,
		);
	}

	return body;
}

function extractSetCookies(response) {
	if (typeof response.headers.getSetCookie === "function") {
		return response.headers.getSetCookie();
	}

	const raw = response.headers.get("set-cookie");
	if (!raw) {
		return [];
	}

	return raw.split(/,(?=\s*[A-Za-z0-9!#$%&'*+.^_`|~-]+=)/);
}

function extractCookieValue(response, cookieName) {
	const matcher = new RegExp(`${cookieName}=([^;]+)`);
	for (const cookie of extractSetCookies(response)) {
		const match = cookie.match(matcher);
		if (match) {
			return decodeURIComponent(match[1]);
		}
	}

	return null;
}

async function apiRequest(path, init = {}) {
	const response = await fetch(`${config.baseUrl}${path}`, init);
	return readJson(response);
}

async function checkAuthState() {
	return apiRequest("/api/v1/auth/check", {
		method: "POST",
	});
}

async function setupUser() {
	return apiRequest("/api/v1/auth/setup", {
		method: "POST",
		headers: {
			"Content-Type": "application/json",
		},
		body: JSON.stringify({
			username: config.username,
			email: config.email,
			password: config.password,
		}),
	});
}

async function login() {
	const response = await fetch(`${config.baseUrl}/api/v1/auth/login`, {
		method: "POST",
		headers: {
			"Content-Type": "application/json",
		},
		body: JSON.stringify({
			identifier: config.username,
			password: config.password,
		}),
	});

	if (response.status === 401) {
		return null;
	}

	const text = await response.text();
	let body;
	try {
		body = text ? JSON.parse(text) : null;
	} catch {
		body = text;
	}

	if (!response.ok) {
		return {
			status: response.status,
			body,
		};
	}

	const accessToken = extractCookieValue(response, "aster_access");
	if (!accessToken) {
		throw new Error("login succeeded but access cookie was missing");
	}

	return {
		accessToken,
		expiresIn: body?.data?.expires_in ?? 0,
		status: response.status,
		body,
	};
}

async function register() {
	return apiRequest("/api/v1/auth/register", {
		method: "POST",
		headers: {
			"Content-Type": "application/json",
		},
		body: JSON.stringify({
			username: config.username,
			email: config.email,
			password: config.password,
		}),
	});
}

async function ensureAuth() {
	const authState = await checkAuthState();
	if (!authState.data.has_users) {
		await setupUser();
	}

	const existing = await login();
	if (existing) {
		if (existing.accessToken) {
			return existing;
		}
		if (
			existing.status === 403 &&
			existing.body?.msg === "account pending activation"
		) {
			throw new Error(
				`benchmark user ${config.username} exists but is pending activation; either confirm the account first or point the seed script at a fresh benchmark database`,
			);
		}
		throw new Error(
			`benchmark login failed: ${JSON.stringify(existing.body ?? existing)}`,
		);
	}

	if (!authState.data.allow_user_registration) {
		throw new Error(
			`benchmark user ${config.username} is missing and self-registration is disabled`,
		);
	}

	try {
		await register();
	} catch (error) {
		throw new Error(
			`failed to register benchmark user ${config.username}: ${error.message}`,
		);
	}

	const created = await login();
	if (created?.accessToken) {
		return created;
	}
	if (
		created?.status === 403 &&
		created.body?.msg === "account pending activation"
	) {
		throw new Error(
			`benchmark user ${config.username} was created but now requires activation; disable registration verification for the benchmark environment or use a pre-activated account`,
		);
	}

	throw new Error(
		`benchmark user registration succeeded but login still failed: ${JSON.stringify(created)}`,
	);
}

function bearerHeaders(token, extra = {}) {
	return {
		Authorization: `Bearer ${token}`,
		...extra,
	};
}

async function listFolder(token, folderId = null, query = {}) {
	const path =
		folderId === null
			? `/api/v1/folders${encodeQuery(query)}`
			: `/api/v1/folders/${folderId}${encodeQuery(query)}`;
	return apiRequest(path, {
		headers: bearerHeaders(token),
	});
}

async function createFolder(token, name, parentId = null) {
	return apiRequest("/api/v1/folders", {
		method: "POST",
		headers: bearerHeaders(token, {
			"Content-Type": "application/json",
		}),
		body: JSON.stringify({
			name,
			parent_id: parentId,
		}),
	});
}

async function ensureRootFolder(token, name) {
	const body = await listFolder(token, null, {
		folder_limit: 1000,
		file_limit: 0,
	});
	const existing = body.data.folders.find((folder) => folder.name === name);
	if (existing) {
		return existing.id;
	}

	const created = await createFolder(token, name, null);
	return created.data.id;
}

async function findFileInFolder(token, folderId, filename) {
	let cursorValue = null;
	let cursorId = null;

	for (;;) {
		const body = await listFolder(token, folderId, {
			folder_limit: 0,
			file_limit: 1000,
			sort_by: "name",
			sort_order: "asc",
			file_after_value: cursorValue,
			file_after_id: cursorId,
		});
		const existing = body.data.files.find((file) => file.name === filename);
		if (existing) {
			return existing.id;
		}

		if (!body.data.next_file_cursor) {
			return null;
		}

		cursorValue = body.data.next_file_cursor.value;
		cursorId = body.data.next_file_cursor.id;
	}
}

async function uploadTextFile(token, folderId, filename, content) {
	const form = new FormData();
	form.append("file", new Blob([content], { type: "text/plain" }), filename);

	return apiRequest(
		`/api/v1/files/upload${encodeQuery({
			folder_id: folderId,
		})}`,
		{
			method: "POST",
			headers: bearerHeaders(token),
			body: form,
		},
	);
}

async function uploadBinaryFile(token, folderId, filename, content, mimeType) {
	const form = new FormData();
	form.append("file", new Blob([content], { type: mimeType }), filename);

	return apiRequest(
		`/api/v1/files/upload${encodeQuery({
			folder_id: folderId,
		})}`,
		{
			method: "POST",
			headers: bearerHeaders(token),
			body: form,
		},
	);
}

async function listFolderCount(token, folderId) {
	const body = await listFolder(token, folderId, {
		folder_limit: 0,
		file_limit: 1,
		sort_by: "name",
		sort_order: "asc",
	});
	return body.data.files_total;
}

function listFileName(size, index) {
	const ordinal = String(index + 1).padStart(5, "0");
	if ((index + 1) % 20 === 0) {
		return `${config.searchTerm}-report-${size}-${ordinal}.txt`;
	}

	return `payload-${size}-${ordinal}.txt`;
}

function listFileBody(size, index) {
	if ((index + 1) % 20 === 0) {
		return `${config.searchTerm} size=${size} index=${index + 1}`;
	}

	return `payload size=${size} index=${index + 1}`;
}

async function seedListFolder(token, size) {
	const folderName = `${LIST_FOLDER_PREFIX}-${size}`;
	const folderId = await ensureRootFolder(token, folderName);
	const existingCount = await listFolderCount(token, folderId);

	if (existingCount >= size) {
		console.log(
			`[seed] ${folderName}: already has ${existingCount} files, skipping`,
		);
		return folderId;
	}

	console.log(
		`[seed] ${folderName}: creating ${size - existingCount} files (${existingCount}/${size})`,
	);

	let cursor = existingCount;
	while (cursor < size) {
		const batchIndices = [];
		for (
			let i = 0;
			i < config.uploadConcurrency && cursor + i < size;
			i += 1
		) {
			batchIndices.push(cursor + i);
		}

		await Promise.all(
			batchIndices.map((index) =>
				uploadTextFile(
					token,
					folderId,
					listFileName(size, index),
					listFileBody(size, index),
				),
			),
		);

		cursor += batchIndices.length;
		if (cursor % 500 === 0 || cursor === size) {
			console.log(`[seed] ${folderName}: ${cursor}/${size}`);
		}
	}

	return folderId;
}

async function ensureDownloadFixture(token) {
	const folderId = await ensureRootFolder(token, DOWNLOAD_FOLDER);
	const existingId = await findFileInFolder(token, folderId, DOWNLOAD_FILE);
	if (existingId) {
		console.log(`[seed] ${DOWNLOAD_FOLDER}: fixture already exists`);
		return { folderId, fileId: existingId };
	}

	const payload = "D".repeat(config.downloadBytes);
	const created = await uploadBinaryFile(
		token,
		folderId,
		DOWNLOAD_FILE,
		payload,
		"application/octet-stream",
	);
	console.log(
		`[seed] ${DOWNLOAD_FOLDER}: created ${DOWNLOAD_FILE} (${config.downloadBytes} bytes)`,
	);
	return { folderId, fileId: created.data.id };
}

async function listWebdavAccounts(token) {
	return apiRequest("/api/v1/webdav-accounts?limit=100&offset=0", {
		headers: bearerHeaders(token),
	});
}

async function createWebdavAccount(token, rootFolderId) {
	return apiRequest("/api/v1/webdav-accounts", {
		method: "POST",
		headers: bearerHeaders(token, {
			"Content-Type": "application/json",
		}),
		body: JSON.stringify({
			username: config.webdavUsername,
			password: config.webdavPassword,
			root_folder_id: rootFolderId,
		}),
	});
}

async function ensureWebdavFixture(token) {
	const rootFolderId = await ensureRootFolder(token, WEBDAV_ROOT_FOLDER);
	const accounts = await listWebdavAccounts(token);
	const existing = accounts.data.items.find(
		(item) => item.username === config.webdavUsername,
	);

	if (existing) {
		console.log(`[seed] webdav account ${config.webdavUsername}: already exists`);
		return { rootFolderId, accountId: existing.id };
	}

	const created = await createWebdavAccount(token, rootFolderId);
	console.log(`[seed] webdav account ${config.webdavUsername}: created`);
	return { rootFolderId, accountId: created.data.id };
}

async function main() {
	console.log(`[seed] base url: ${config.baseUrl}`);
	const auth = await ensureAuth();
	console.log(
		`[seed] benchmark user ready: ${config.username} (ttl=${auth.expiresIn}s)`,
	);

	const rootToken = auth.accessToken;
	const listFolders = {};
	for (const size of config.listSizes) {
		listFolders[size] = await seedListFolder(rootToken, size);
	}

	const download = await ensureDownloadFixture(rootToken);
	const batchTargetId = await ensureRootFolder(rootToken, BATCH_TARGET_FOLDER);
	const webdav = await ensureWebdavFixture(rootToken);

	console.log("[seed] completed");
	console.log(
		JSON.stringify(
			{
				user: config.username,
				list_folders: listFolders,
				download,
				batch_target_id: batchTargetId,
				webdav,
			},
			null,
			2,
		),
	);
}

await main();
