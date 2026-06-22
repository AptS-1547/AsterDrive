import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { OpenWithOption } from "./types";
import { usePreviewSessionResources } from "./usePreviewSessionResources";

const archiveOption: OpenWithOption = {
	icon: "Archive",
	key: "builtin.archive",
	labelKey: "open_with_archive",
	mode: "archive",
};
const wopiOption: OpenWithOption = {
	icon: "FileText",
	key: "onlyoffice",
	labelKey: "open_with_onlyoffice",
	mode: "wopi",
};

describe("usePreviewSessionResources", () => {
	it("uses the latest archive manifest loader through a stable active loader", async () => {
		const initialLoader = vi.fn(async () => ({ entries: [] }));
		const latestLoader = vi.fn(async () => ({
			entries: [{ name: "latest.txt" }],
		}));
		const { rerender, result } = renderHook(
			(props: {
				activeOption: OpenWithOption;
				archiveManifestLoader: typeof initialLoader;
			}) => usePreviewSessionResources({ fileId: 7, open: true, ...props }),
			{
				initialProps: {
					activeOption: archiveOption,
					archiveManifestLoader: initialLoader,
				},
			},
		);
		const activeLoader = result.current.activeArchiveManifestLoader;

		rerender({
			activeOption: archiveOption,
			archiveManifestLoader: latestLoader,
		});

		await expect(
			activeLoader?.({ signal: new AbortController().signal }),
		).resolves.toEqual({
			entries: [{ name: "latest.txt" }],
		});
		expect(initialLoader).not.toHaveBeenCalled();
		expect(latestLoader).toHaveBeenCalledWith({
			signal: expect.any(AbortSignal),
		});
	});

	it("creates and reuses WOPI session resources for the same file and app", async () => {
		const launchWopiSession = vi.fn(async (appKey: string) => ({
			app_key: appKey,
			expires_at: "2026-01-01T00:00:00Z",
			launch_url: "https://office.example/launch",
			token: "token",
		}));
		const { rerender, result } = renderHook(
			(props: { fileId: number }) =>
				usePreviewSessionResources({
					activeOption: wopiOption,
					fileId: props.fileId,
					launchWopiSession,
					open: true,
				}),
			{ initialProps: { fileId: 7 } },
		);
		const firstResource = result.current.wopiSessionResource;

		rerender({ fileId: 7 });
		expect(result.current.wopiSessionResource).toBe(firstResource);

		await act(async () => {
			await result.current.launchWopiSession?.();
		});
		expect(launchWopiSession).toHaveBeenCalledWith("onlyoffice");

		rerender({ fileId: 8 });
		expect(result.current.wopiSessionResource).not.toBe(firstResource);
	});

	it("does not expose inactive session resources", () => {
		const { result } = renderHook(() =>
			usePreviewSessionResources({
				activeOption: null,
				fileId: 7,
				open: true,
			}),
		);

		expect(result.current.activeArchiveManifestLoader).toBeUndefined();
		expect(result.current.launchWopiSession).toBeNull();
		expect(result.current.wopiSessionResource).toBeNull();
	});
});
