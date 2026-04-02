import {
	PERSONAL_WORKSPACE,
	type Workspace,
	workspaceEquals,
} from "@/lib/workspace";

/**
 * 断点续传持久化层
 * 将 chunked upload session 元数据存入 localStorage，刷新后可恢复。
 */

export interface ResumableSession {
	uploadId: string;
	filename: string;
	totalSize: number;
	totalChunks: number;
	chunkSize: number;
	baseFolderId: number | null;
	baseFolderName: string;
	relativePath: string | null;
	savedAt: number;
	workspace?: Workspace;
	/** "chunked" (本地分片) 或 "presigned_multipart" (S3 分片直传) */
	mode?: "chunked" | "presigned_multipart";
	/** S3 multipart: 已上传 part 的 {partNumber, etag} */
	completedParts?: { part_number: number; etag: string }[];
}

const STORAGE_KEY = "aster_resumable_uploads";
/** 23h — 留 1h 余量，服务器 session 24h 过期 */
const MAX_AGE_MS = 23 * 60 * 60 * 1000;

function readAll(): ResumableSession[] {
	try {
		const raw = localStorage.getItem(STORAGE_KEY);
		if (!raw) return [];
		return JSON.parse(raw) as ResumableSession[];
	} catch {
		return [];
	}
}

function writeAll(sessions: ResumableSession[]): void {
	if (sessions.length === 0) {
		localStorage.removeItem(STORAGE_KEY);
	} else {
		localStorage.setItem(STORAGE_KEY, JSON.stringify(sessions));
	}
}

function normalizeWorkspace(workspace?: Workspace): Workspace {
	return workspace ?? PERSONAL_WORKSPACE;
}

/** 保存一个 chunked upload session（init 成功后调用） */
export function saveSession(session: ResumableSession): void {
	const all = readAll().filter((s) => s.uploadId !== session.uploadId);
	all.push({
		...session,
		workspace: normalizeWorkspace(session.workspace),
	});
	writeAll(all);
}

/** 移除一个 session（complete/cancel/永久删除时调用） */
export function removeSession(uploadId: string): void {
	writeAll(readAll().filter((s) => s.uploadId !== uploadId));
}

/** 追加已完成的 part 到 session（S3 multipart 每上传完一个 part 调用） */
export function appendCompletedPart(
	uploadId: string,
	part: { part_number: number; etag: string },
): void {
	const all = readAll();
	const session = all.find((s) => s.uploadId === uploadId);
	if (!session) return;
	const parts = session.completedParts ?? [];
	if (!parts.some((p) => p.part_number === part.part_number)) {
		parts.push(part);
		session.completedParts = parts;
		writeAll(all);
	}
}

/** 加载所有未过期的 session，自动清理过期的 */
export function loadSessions(workspace?: Workspace): ResumableSession[] {
	const now = Date.now();
	const all = readAll();
	const valid = all.filter((s) => now - s.savedAt < MAX_AGE_MS);
	if (valid.length !== all.length) {
		writeAll(valid);
	}
	if (!workspace) return valid;
	return valid.filter((session) =>
		workspaceEquals(normalizeWorkspace(session.workspace), workspace),
	);
}

/** 清空所有 session */
export function clearAllSessions(): void {
	localStorage.removeItem(STORAGE_KEY);
}
