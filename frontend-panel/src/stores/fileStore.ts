import { create } from "zustand";
import { createClipboardSlice } from "@/stores/fileStore/clipboardSlice";
import { createCrudSlice } from "@/stores/fileStore/crudSlice";
import { createNavigationSlice } from "@/stores/fileStore/navigationSlice";
import { createPreferencesSlice } from "@/stores/fileStore/preferencesSlice";
import { createRequestSlice } from "@/stores/fileStore/requestSlice";
import { createSelectionSlice } from "@/stores/fileStore/selectionSlice";
import type { FileState } from "@/stores/fileStore/types";

export type {
	BreadcrumbItem,
	BrowserOpenMode,
	Clipboard,
	SortBy,
	SortOrder,
	ViewMode,
} from "@/stores/fileStore/types";

export const useFileStore = create<FileState>()((...args) => ({
	...createRequestSlice(...args),
	...createNavigationSlice(...args),
	...createPreferencesSlice(...args),
	...createSelectionSlice(...args),
	...createClipboardSlice(...args),
	...createCrudSlice(...args),
}));
