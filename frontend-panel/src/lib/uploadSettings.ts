import { STORAGE_KEYS } from "@/config/app";
import { readLocalStorage, writeLocalStorage } from "@/lib/storage";

export const DEFAULT_UPLOAD_CONCURRENCY = 2;
export const MIN_UPLOAD_CONCURRENCY = 1;
export const MAX_UPLOAD_CONCURRENCY = 8;
export const DEFAULT_UPLOAD_AUTO_CLEAR_COMPLETED = false;

export interface UploadSettings {
	autoClearCompleted: boolean;
	concurrency: number;
}

export function normalizeUploadConcurrency(
	value: number,
	fallback = DEFAULT_UPLOAD_CONCURRENCY,
) {
	if (!Number.isFinite(value)) return fallback;
	const integer = Math.trunc(value);
	if (integer < MIN_UPLOAD_CONCURRENCY) return MIN_UPLOAD_CONCURRENCY;
	if (integer > MAX_UPLOAD_CONCURRENCY) return MAX_UPLOAD_CONCURRENCY;
	return integer;
}

function readStoredUploadConcurrency() {
	const raw = readLocalStorage(STORAGE_KEYS.uploadConcurrency);
	if (raw === null) return DEFAULT_UPLOAD_CONCURRENCY;
	const parsed = Number.parseInt(raw, 10);
	return normalizeUploadConcurrency(parsed);
}

function readStoredUploadAutoClearCompleted() {
	const raw = readLocalStorage(STORAGE_KEYS.uploadAutoClearCompleted);
	if (raw === null) return DEFAULT_UPLOAD_AUTO_CLEAR_COMPLETED;
	return raw === "true";
}

export function readUploadSettings(): UploadSettings {
	return {
		autoClearCompleted: readStoredUploadAutoClearCompleted(),
		concurrency: readStoredUploadConcurrency(),
	};
}

export function writeUploadConcurrency(value: number) {
	writeLocalStorage(
		STORAGE_KEYS.uploadConcurrency,
		String(normalizeUploadConcurrency(value)),
	);
}

export function writeUploadAutoClearCompleted(value: boolean) {
	writeLocalStorage(STORAGE_KEYS.uploadAutoClearCompleted, String(value));
}
