import { DRAG_MIME } from "@/lib/constants";

export interface InternalDragData {
	fileIds: number[];
	folderIds: number[];
}

function sanitizeIds(value: unknown): number[] {
	if (!Array.isArray(value)) return [];
	return value.filter((id): id is number => Number.isInteger(id) && id > 0);
}

export function hasInternalDragData(
	dataTransfer: DataTransfer | null,
): boolean {
	return dataTransfer?.types.includes(DRAG_MIME) ?? false;
}

export function readInternalDragData(
	dataTransfer: DataTransfer | null,
): InternalDragData | null {
	if (!dataTransfer || !hasInternalDragData(dataTransfer)) return null;

	const raw = dataTransfer.getData(DRAG_MIME);
	if (!raw) return null;

	try {
		const parsed = JSON.parse(raw) as Partial<InternalDragData>;
		const data = {
			fileIds: sanitizeIds(parsed.fileIds),
			folderIds: sanitizeIds(parsed.folderIds),
		};

		if (data.fileIds.length === 0 && data.folderIds.length === 0) return null;
		return data;
	} catch {
		return null;
	}
}

export function writeInternalDragData(
	dataTransfer: DataTransfer,
	data: InternalDragData,
) {
	dataTransfer.setData(DRAG_MIME, JSON.stringify(data));
	dataTransfer.effectAllowed = "move";
}
