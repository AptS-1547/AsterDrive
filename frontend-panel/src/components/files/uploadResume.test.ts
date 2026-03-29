import { describe, expect, it } from "vitest";
import {
	CHUNK_PROCESSING_PROGRESS,
	getProcessingProgress,
	getResumePlan,
	S3_PROCESSING_PROGRESS,
	type UploadMode,
} from "@/components/files/uploadResume";
import type { UploadSessionStatus } from "@/types/api";

describe("uploadResume", () => {
	it("maps chunked session statuses to the expected resume plan", () => {
		expect(getResumePlan("chunked", "uploading")).toBe("upload");
		expect(getResumePlan("chunked", "assembling")).toBe("complete");
		expect(getResumePlan("chunked", "completed")).toBe("complete");
		expect(getResumePlan("chunked", "failed")).toBe("restart");
		expect(getResumePlan("chunked", "presigned")).toBe("restart");
	});

	it("maps multipart presigned statuses to the expected resume plan", () => {
		expect(getResumePlan("presigned_multipart", "presigned")).toBe("upload");
		expect(getResumePlan("presigned_multipart", "assembling")).toBe("complete");
		expect(getResumePlan("presigned_multipart", "completed")).toBe("complete");
		expect(getResumePlan("presigned_multipart", "uploading")).toBe("restart");
		expect(getResumePlan("presigned_multipart", "failed")).toBe("restart");
	});

	it("never resumes direct or single-request presigned uploads", () => {
		const statuses: UploadSessionStatus[] = [
			"uploading",
			"assembling",
			"completed",
			"failed",
			"presigned",
		];

		for (const mode of ["direct", "presigned"] satisfies UploadMode[]) {
			for (const status of statuses) {
				expect(getResumePlan(mode, status)).toBe("restart");
			}
		}
	});

	it("uses chunk processing progress only for chunked assembly", () => {
		expect(getProcessingProgress("chunked")).toBe(CHUNK_PROCESSING_PROGRESS);
		expect(getProcessingProgress("presigned_multipart")).toBe(
			S3_PROCESSING_PROGRESS,
		);
		expect(getProcessingProgress("presigned")).toBe(S3_PROCESSING_PROGRESS);
		expect(getProcessingProgress("direct")).toBe(S3_PROCESSING_PROGRESS);
		expect(getProcessingProgress(null)).toBe(S3_PROCESSING_PROGRESS);
	});
});
