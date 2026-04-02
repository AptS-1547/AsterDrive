import { beforeEach, describe, expect, it, vi } from "vitest";
import {
	appendCompletedPart,
	clearAllSessions,
	loadSessions,
	type ResumableSession,
	removeSession,
	saveSession,
} from "@/lib/uploadPersistence";
import type { Workspace } from "@/lib/workspace";

function createSession(
	overrides: Partial<ResumableSession> = {},
): ResumableSession {
	return {
		uploadId: "upload-1",
		filename: "hello.txt",
		totalSize: 12,
		totalChunks: 3,
		chunkSize: 4,
		baseFolderId: 42,
		baseFolderName: "Projects",
		relativePath: null,
		savedAt: Date.now(),
		mode: "chunked",
		...overrides,
	};
}

const TEAM_WORKSPACE: Workspace = { kind: "team", teamId: 9 };

describe("uploadPersistence", () => {
	beforeEach(() => {
		localStorage.clear();
		vi.restoreAllMocks();
	});

	it("saves, replaces, and removes sessions by upload id", () => {
		saveSession(createSession());
		saveSession(
			createSession({
				uploadId: "upload-2",
				filename: "world.txt",
			}),
		);
		saveSession(
			createSession({
				uploadId: "upload-1",
				filename: "hello-v2.txt",
			}),
		);

		expect(loadSessions()).toEqual([
			expect.objectContaining({
				uploadId: "upload-2",
				filename: "world.txt",
			}),
			expect.objectContaining({
				uploadId: "upload-1",
				filename: "hello-v2.txt",
			}),
		]);

		removeSession("upload-2");

		expect(loadSessions()).toEqual([
			expect.objectContaining({
				uploadId: "upload-1",
				filename: "hello-v2.txt",
			}),
		]);
	});

	it("tracks completed parts without duplicating the same part number", () => {
		saveSession(
			createSession({
				mode: "presigned_multipart",
				completedParts: [{ part_number: 1, etag: "etag-1" }],
			}),
		);

		appendCompletedPart("upload-1", {
			part_number: 2,
			etag: "etag-2",
		});
		appendCompletedPart("upload-1", {
			part_number: 2,
			etag: "etag-2-duplicate",
		});

		expect(loadSessions()).toEqual([
			expect.objectContaining({
				completedParts: [
					{ part_number: 1, etag: "etag-1" },
					{ part_number: 2, etag: "etag-2" },
				],
			}),
		]);
	});

	it("drops expired sessions when loading", () => {
		const now = 10_000_000;
		vi.spyOn(Date, "now").mockReturnValue(now);

		saveSession(
			createSession({
				uploadId: "fresh",
				savedAt: now - (23 * 60 * 60 * 1000 - 1),
			}),
		);
		saveSession(
			createSession({
				uploadId: "expired",
				savedAt: now - (23 * 60 * 60 * 1000 + 1),
			}),
		);

		expect(loadSessions()).toEqual([
			expect.objectContaining({
				uploadId: "fresh",
			}),
		]);
		expect(loadSessions()).toEqual([
			expect.objectContaining({
				uploadId: "fresh",
			}),
		]);
	});

	it("clears all persisted sessions", () => {
		saveSession(createSession());

		clearAllSessions();

		expect(loadSessions()).toEqual([]);
	});

	it("filters sessions by workspace when requested", () => {
		saveSession(createSession({ uploadId: "personal-1" }));
		saveSession(
			createSession({
				uploadId: "team-1",
				workspace: TEAM_WORKSPACE,
			}),
		);

		expect(loadSessions()).toHaveLength(2);
		expect(loadSessions(TEAM_WORKSPACE)).toEqual([
			expect.objectContaining({
				uploadId: "team-1",
			}),
		]);
	});
});
