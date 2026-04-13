import {
	lazy,
	type ReactNode,
	Suspense,
	useCallback,
	useEffect,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { useTranslation } from "react-i18next";
import { PreviewAppIcon } from "@/components/common/PreviewAppIcon";
import { FileTypeIcon } from "@/components/files/FileTypeIcon";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Icon } from "@/components/ui/icon";
import { ScrollArea } from "@/components/ui/scroll-area";
import { formatBytes } from "@/lib/format";
import { normalizeTablePreviewDelimiter } from "@/lib/tablePreview";
import { cn } from "@/lib/utils";
import { fileService } from "@/services/fileService";
import { usePreviewAppStore } from "@/stores/previewAppStore";
import type {
	FileInfo,
	FileListItem,
	PreviewLinkInfo,
	WopiLaunchSession,
} from "@/types/api";
import { BlobMediaPreview } from "./BlobMediaPreview";
import {
	detectFilePreviewProfile,
	getFileExtension,
} from "./file-capabilities";
import { resolveOpenWithOptionLabel } from "./openWithLabel";
import { PreviewLoadingState } from "./PreviewLoadingState";
import { PreviewUnavailable } from "./PreviewUnavailable";
import type { OpenWithMode, OpenWithOption } from "./types";
import { UnsavedChangesGuard } from "./UnsavedChangesGuard";
import { UrlTemplatePreview } from "./UrlTemplatePreview";
import { VideoPreview } from "./VideoPreview";
import { getVideoBrowserOpenWithOption } from "./video-browser-config";
import { WopiPreview } from "./WopiPreview";

const PdfPreview = lazy(async () => {
	const module = await import("./PdfPreview");
	return { default: module.PdfPreview };
});

const MarkdownPreview = lazy(async () => {
	const module = await import("./MarkdownPreview");
	return { default: module.MarkdownPreview };
});

const CsvTablePreview = lazy(async () => {
	const module = await import("./CsvTablePreview");
	return { default: module.CsvTablePreview };
});

const JsonPreview = lazy(async () => {
	const module = await import("./JsonPreview");
	return { default: module.JsonPreview };
});

const XmlPreview = lazy(async () => {
	const module = await import("./XmlPreview");
	return { default: module.XmlPreview };
});

const TextCodePreview = lazy(async () => {
	const module = await import("./TextCodePreview");
	return { default: module.TextCodePreview };
});

const MORE_METHODS_EXPAND_DURATION_MS = 220;
const MORE_METHODS_COLLAPSE_DURATION_MS = 160;

function AnimatedCollapsible({
	children,
	className,
	contentClassName,
	open,
}: {
	children: ReactNode;
	className?: string;
	contentClassName?: string;
	open: boolean;
}) {
	const containerRef = useRef<HTMLDivElement | null>(null);
	const contentRef = useRef<HTMLDivElement | null>(null);
	const [mounted, setMounted] = useState(open);

	useEffect(() => {
		if (typeof window === "undefined") {
			setMounted(open);
			return;
		}

		if (open) {
			setMounted(true);
		}
	}, [open]);

	useLayoutEffect(() => {
		if (typeof window === "undefined" || !mounted) {
			return;
		}

		const container = containerRef.current;
		const content = contentRef.current;
		if (!container || !content) {
			return;
		}

		const prefersReducedMotion =
			typeof window.matchMedia === "function" &&
			window.matchMedia("(prefers-reduced-motion: reduce)").matches;
		const duration = prefersReducedMotion
			? 0
			: open
				? MORE_METHODS_EXPAND_DURATION_MS
				: MORE_METHODS_COLLAPSE_DURATION_MS;
		let frameA: number | null = null;
		let frameB: number | null = null;
		let timer: number | null = null;
		const fullHeight = `${content.scrollHeight}px`;

		container.style.overflow = "hidden";
		container.style.transitionProperty = "max-height, opacity, transform";
		container.style.transitionDuration = `${duration}ms`;
		container.style.transitionTimingFunction = open
			? "cubic-bezier(0.22, 1, 0.36, 1)"
			: "cubic-bezier(0.4, 0, 1, 1)";

		if (open) {
			container.style.maxHeight = "0px";
			container.style.opacity = "0";
			container.style.transform = "translateY(-6px)";
			frameA = window.requestAnimationFrame(() => {
				frameB = window.requestAnimationFrame(() => {
					container.style.maxHeight = fullHeight;
					container.style.opacity = "1";
					container.style.transform = "translateY(0)";
				});
			});
			timer = window.setTimeout(() => {
				container.style.maxHeight = "none";
				container.style.opacity = "1";
				container.style.transform = "translateY(0)";
			}, duration);
		} else {
			container.style.maxHeight = fullHeight;
			container.style.opacity = "1";
			container.style.transform = "translateY(0)";
			frameA = window.requestAnimationFrame(() => {
				container.style.maxHeight = "0px";
				container.style.opacity = "0";
				container.style.transform = "translateY(-6px)";
			});
			timer = window.setTimeout(() => {
				setMounted(false);
			}, duration);
		}

		return () => {
			if (frameA !== null) {
				window.cancelAnimationFrame(frameA);
			}
			if (frameB !== null) {
				window.cancelAnimationFrame(frameB);
			}
			if (timer !== null) {
				window.clearTimeout(timer);
			}
		};
	}, [mounted, open]);

	if (!mounted) {
		return null;
	}

	return (
		<div
			ref={containerRef}
			aria-hidden={!open}
			className={cn("overflow-hidden", className)}
		>
			<div ref={contentRef} className={cn("min-h-0", contentClassName)}>
				{children}
			</div>
		</div>
	);
}

interface FilePreviewDialogProps {
	file: FileInfo | FileListItem;
	onClose: () => void;
	onFileUpdated?: () => void;
	downloadPath?: string;
	editable?: boolean;
	previewLinkFactory?: () => Promise<PreviewLinkInfo>;
	wopiSessionFactory?: (appKey: string) => Promise<WopiLaunchSession>;
	openMode?: "auto" | "direct" | "picker";
}

function getEmbeddedOptionMode(option: OpenWithOption | null) {
	if (!option) {
		return "new_tab";
	}

	if (option.mode !== "url_template" && option.mode !== "wopi") {
		return "iframe";
	}

	return option.config?.mode === "new_tab" ? "new_tab" : "iframe";
}

export function FilePreviewDialog({
	file,
	onClose,
	onFileUpdated,
	downloadPath,
	editable = true,
	previewLinkFactory,
	wopiSessionFactory,
	openMode = "auto",
}: FilePreviewDialogProps) {
	const { i18n, t } = useTranslation(["core", "files"]);
	const previewApps = usePreviewAppStore((state) => state.config);
	const previewAppsLoaded = usePreviewAppStore((state) => state.isLoaded);
	const loadPreviewApps = usePreviewAppStore((state) => state.load);
	const resolvedDownloadPath =
		downloadPath ?? fileService.downloadPath(file.id);

	useEffect(() => {
		if (previewAppsLoaded) return;
		void loadPreviewApps();
	}, [loadPreviewApps, previewAppsLoaded]);

	const baseProfile = useMemo(() => {
		if (!previewAppsLoaded) return null;
		return detectFilePreviewProfile(file, previewApps);
	}, [file, previewApps, previewAppsLoaded]);

	const customVideoBrowserOption = useMemo(
		() => getVideoBrowserOpenWithOption(),
		[],
	);

	const profile = useMemo(() => {
		if (!baseProfile) return null;
		if (
			baseProfile.category !== "video" ||
			!customVideoBrowserOption ||
			baseProfile.options.some(
				(option) => option.key === customVideoBrowserOption.key,
			)
		) {
			return baseProfile;
		}

		return {
			...baseProfile,
			options: [...baseProfile.options, customVideoBrowserOption],
			allOptions: [
				...(baseProfile.allOptions ?? baseProfile.options),
				customVideoBrowserOption,
			],
		};
	}, [baseProfile, customVideoBrowserOption]);

	const isOptionAvailable = useCallback(
		(option: OpenWithOption) =>
			option.mode !== "wopi" || Boolean(wopiSessionFactory),
		[wopiSessionFactory],
	);

	const allOptions = useMemo(
		() =>
			(profile?.allOptions ?? profile?.options ?? []).filter(isOptionAvailable),
		[isOptionAvailable, profile],
	);
	const visibleOptions = useMemo(() => {
		if (!profile || profile.options.length === 0) {
			return allOptions;
		}

		const nextVisibleOptions = profile.options.filter(isOptionAvailable);
		return nextVisibleOptions.length > 0 ? nextVisibleOptions : allOptions;
	}, [allOptions, isOptionAvailable, profile]);
	const hiddenOptions = useMemo(
		() =>
			allOptions.filter(
				(option) =>
					!visibleOptions.some((candidate) => candidate.key === option.key),
			),
		[allOptions, visibleOptions],
	);

	const preferredMode = useMemo(() => {
		if (!profile) return null;
		if (
			profile.defaultMode &&
			allOptions.some((option) => option.key === profile.defaultMode)
		) {
			return profile.defaultMode;
		}
		return allOptions[0]?.key ?? null;
	}, [allOptions, profile]);

	const [mode, setMode] = useState<OpenWithMode | null>(null);
	const previousFileIdRef = useRef(file.id);
	const [hasConfirmedInitialMode, setHasConfirmedInitialMode] = useState(false);
	useEffect(() => {
		const hasFileChanged = previousFileIdRef.current !== file.id;
		if (hasFileChanged) {
			previousFileIdRef.current = file.id;
			setHasConfirmedInitialMode(false);
		}
		setMode(preferredMode);
	}, [file.id, preferredMode]);

	const [isDirty, setIsDirty] = useState(false);
	const [confirmOpen, setConfirmOpen] = useState(false);
	const activeMode = mode ?? preferredMode;
	const [showAllOpenMethods, setShowAllOpenMethods] = useState(false);
	useEffect(() => {
		setShowAllOpenMethods(
			Boolean(
				activeMode && hiddenOptions.some((option) => option.key === activeMode),
			),
		);
	}, [activeMode, hiddenOptions]);
	const activeOption = useMemo(() => {
		if (!profile || !activeMode) return null;
		return allOptions.find((option) => option.key === activeMode) ?? null;
	}, [activeMode, allOptions, profile]);

	const getOptionLabel = useCallback(
		(option: OpenWithOption) =>
			resolveOpenWithOptionLabel(option, i18n?.language, (key) =>
				t(`files:${key}`),
			),
		[i18n?.language, t],
	);
	const activeWopiSessionFactory = useCallback(() => {
		if (!activeOption || activeOption.mode !== "wopi" || !wopiSessionFactory) {
			return Promise.reject(new Error("wopi session factory unavailable"));
		}

		return wopiSessionFactory(activeOption.key);
	}, [activeOption, wopiSessionFactory]);
	const showOpenMethodChooser =
		previewAppsLoaded &&
		(openMode === "picker"
			? allOptions.length > 0
			: openMode === "direct"
				? false
				: allOptions.length > 1) &&
		!hasConfirmedInitialMode;

	const usesInnerScroll =
		activeOption?.mode === "pdf" ||
		activeOption?.mode === "table" ||
		((activeOption?.mode === "url_template" || activeOption?.mode === "wopi") &&
			getEmbeddedOptionMode(activeOption) !== "new_tab");
	const fillsViewportHeight =
		activeOption?.mode === "code" ||
		activeOption?.mode === "formatted" ||
		activeOption?.mode === "markdown" ||
		activeOption?.mode === "pdf" ||
		activeOption?.mode === "table" ||
		((activeOption?.mode === "url_template" || activeOption?.mode === "wopi") &&
			getEmbeddedOptionMode(activeOption) !== "new_tab");
	const previewLoadingState = (
		<PreviewLoadingState
			text={t("files:loading_preview")}
			className="h-full min-h-[16rem]"
		/>
	);

	const closeWithGuard = useCallback(() => {
		if (isDirty) {
			setConfirmOpen(true);
			return;
		}
		onClose();
	}, [isDirty, onClose]);

	const handleOpenMethodSelect = useCallback((nextMode: OpenWithMode) => {
		setMode(nextMode);
		setHasConfirmedInitialMode(true);
	}, []);

	const handleDiscardChanges = useCallback(() => {
		setConfirmOpen(false);
		setIsDirty(false);
		onClose();
	}, [onClose]);

	const body = (() => {
		if (!previewAppsLoaded) {
			return previewLoadingState;
		}
		if (!profile) return <PreviewUnavailable />;
		if (!activeOption) return <PreviewUnavailable />;
		if (activeOption.mode === "pdf") {
			return (
				<Suspense fallback={previewLoadingState}>
					<PdfPreview path={resolvedDownloadPath} fileName={file.name} />
				</Suspense>
			);
		}
		if (activeOption.mode === "image" || activeOption.mode === "audio") {
			return (
				<BlobMediaPreview
					file={file}
					mode={activeOption.mode}
					path={resolvedDownloadPath}
				/>
			);
		}
		if (activeOption.mode === "video") {
			return <VideoPreview file={file} path={resolvedDownloadPath} />;
		}
		if (activeOption.mode === "url_template") {
			return (
				<UrlTemplatePreview
					file={file}
					downloadPath={resolvedDownloadPath}
					label={getOptionLabel(activeOption)}
					rawConfig={activeOption.config ?? null}
					createPreviewLink={previewLinkFactory}
				/>
			);
		}
		if (activeOption.mode === "wopi") {
			if (!wopiSessionFactory) {
				return <PreviewUnavailable />;
			}
			return (
				<WopiPreview
					label={getOptionLabel(activeOption)}
					rawConfig={activeOption.config ?? null}
					createSession={activeWopiSessionFactory}
				/>
			);
		}
		if (activeOption.mode === "markdown") {
			return (
				<Suspense fallback={previewLoadingState}>
					<MarkdownPreview path={resolvedDownloadPath} />
				</Suspense>
			);
		}
		if (activeOption.mode === "table") {
			const delimiter = normalizeTablePreviewDelimiter(
				activeOption.config?.delimiter,
			);

			return (
				<Suspense fallback={previewLoadingState}>
					<CsvTablePreview path={resolvedDownloadPath} delimiter={delimiter} />
				</Suspense>
			);
		}
		if (activeOption.mode === "formatted") {
			const formattedCategory =
				profile.category === "xml" || getFileExtension(file) === "xml"
					? "xml"
					: "json";

			if (formattedCategory === "xml") {
				return (
					<Suspense fallback={previewLoadingState}>
						<XmlPreview path={resolvedDownloadPath} mode="formatted" />
					</Suspense>
				);
			}

			return (
				<Suspense fallback={previewLoadingState}>
					<JsonPreview path={resolvedDownloadPath} />
				</Suspense>
			);
		}
		if (activeOption.mode === "code") {
			return (
				<Suspense fallback={previewLoadingState}>
					<TextCodePreview
						file={file}
						modeLabel={getOptionLabel(activeOption)}
						path={resolvedDownloadPath}
						onFileUpdated={onFileUpdated}
						onDirtyChange={setIsDirty}
						editable={editable}
					/>
				</Suspense>
			);
		}
		return <PreviewUnavailable />;
	})();

	return (
		<>
			{showOpenMethodChooser ? (
				<Dialog open onOpenChange={(open) => !open && onClose()}>
					<DialogContent className="flex max-h-[min(90vh,calc(100vh-2rem))] w-[min(96vw,32rem)] max-w-[min(96vw,32rem)] flex-col gap-0 overflow-hidden p-0 sm:max-w-[min(96vw,32rem)]">
						<DialogHeader className="border-b px-5 py-4">
							<div className="flex items-center gap-3">
								<div className="flex h-10 w-10 items-center justify-center rounded-xl bg-muted text-muted-foreground">
									<FileTypeIcon
										mimeType={file.mime_type}
										fileName={file.name}
										className="h-5 w-5"
									/>
								</div>
								<div className="min-w-0 flex-1">
									<DialogTitle className="truncate">
										{t("files:choose_open_method")}
									</DialogTitle>
									<p className="mt-1 truncate text-sm text-muted-foreground">
										{file.name} · {formatBytes(file.size)}
									</p>
								</div>
							</div>
						</DialogHeader>
						<div className="min-h-0 overflow-y-auto p-4">
							<div className="grid gap-2">
								{visibleOptions.map((option) => {
									const isActive = option.key === activeMode;
									return (
										<Button
											key={option.key}
											variant="ghost"
											className={cn(
												"h-auto justify-start rounded-xl border px-3.5 py-2.5 text-left",
												isActive && "border-primary bg-accent text-foreground",
											)}
											onClick={() => handleOpenMethodSelect(option.key)}
										>
											<div className="flex w-full items-center gap-2.5">
												<div className="flex h-9 w-9 items-center justify-center rounded-lg bg-muted text-muted-foreground">
													<PreviewAppIcon
														icon={option.icon}
														className="h-4 w-4"
													/>
												</div>
												<div className="min-w-0 flex-1">
													<div className="truncate font-medium">
														{getOptionLabel(option)}
													</div>
												</div>
												<Icon
													name={isActive ? "Check" : "CaretRight"}
													className="h-4 w-4 text-muted-foreground"
												/>
											</div>
										</Button>
									);
								})}
								<AnimatedCollapsible open={showAllOpenMethods}>
									<div className="grid gap-2">
										{hiddenOptions.map((option) => {
											const isActive = option.key === activeMode;
											return (
												<Button
													key={option.key}
													variant="ghost"
													className={cn(
														"h-auto justify-start rounded-xl border px-3.5 py-2.5 text-left",
														isActive &&
															"border-primary bg-accent text-foreground",
													)}
													onClick={() => handleOpenMethodSelect(option.key)}
												>
													<div className="flex w-full items-center gap-2.5">
														<div className="flex h-9 w-9 items-center justify-center rounded-lg bg-muted text-muted-foreground">
															<PreviewAppIcon
																icon={option.icon}
																className="h-4 w-4"
															/>
														</div>
														<div className="min-w-0 flex-1">
															<div className="truncate font-medium">
																{getOptionLabel(option)}
															</div>
														</div>
														<Icon
															name={isActive ? "Check" : "CaretRight"}
															className="h-4 w-4 text-muted-foreground"
														/>
													</div>
												</Button>
											);
										})}
									</div>
								</AnimatedCollapsible>
								{!showAllOpenMethods && allOptions.length > 0 ? (
									<Button
										variant="ghost"
										className="h-auto justify-start rounded-xl border border-dashed px-3.5 py-2.5 text-left text-muted-foreground"
										onClick={() => setShowAllOpenMethods(true)}
									>
										<div className="flex w-full items-center gap-2.5">
											<div className="min-w-0 flex-1">
												<div className="font-medium">
													{t("files:more_open_methods")}
												</div>
											</div>
											<Icon name="CaretDown" className="h-4 w-4" />
										</div>
									</Button>
								) : null}
							</div>
						</div>
					</DialogContent>
				</Dialog>
			) : null}
			{showOpenMethodChooser ? null : (
				<Dialog open onOpenChange={(open) => !open && closeWithGuard()}>
					<DialogContent
						showCloseButton={false}
						className={cn(
							"flex max-h-[90vh] w-[min(96vw,1200px)] max-w-[min(96vw,1200px)] flex-col gap-0 overflow-hidden p-0 sm:max-w-[min(96vw,1200px)]",
							fillsViewportHeight && "h-[90vh]",
						)}
					>
						<DialogHeader className="gap-0 border-b px-4 py-3">
							<div className="flex items-center gap-3">
								<div className="flex h-9 w-9 items-center justify-center rounded-lg bg-muted text-muted-foreground">
									<FileTypeIcon
										mimeType={file.mime_type}
										fileName={file.name}
										className="h-5 w-5"
									/>
								</div>
								<div className="min-w-0 flex-1">
									<DialogTitle className="flex items-center gap-2 text-sm font-semibold">
										<span className="min-w-0 truncate">{file.name}</span>
										<span className="shrink-0 text-xs font-normal text-muted-foreground">
											· {formatBytes(file.size)}
										</span>
									</DialogTitle>
								</div>
								<Button variant="ghost" size="icon-sm" onClick={closeWithGuard}>
									<Icon name="X" className="h-4 w-4" />
								</Button>
							</div>
						</DialogHeader>
						{usesInnerScroll ? (
							<div
								className={cn(
									"bg-muted/20 p-3",
									fillsViewportHeight && "min-h-0 flex-1",
								)}
							>
								{body}
							</div>
						) : (
							<ScrollArea
								className={cn(
									"bg-muted/20",
									fillsViewportHeight && "min-h-0 flex-1",
								)}
							>
								<div
									className={cn(
										"w-full p-3",
										fillsViewportHeight && "h-full min-h-full",
									)}
								>
									{body}
								</div>
							</ScrollArea>
						)}
					</DialogContent>
				</Dialog>
			)}
			<UnsavedChangesGuard
				open={confirmOpen}
				onOpenChange={setConfirmOpen}
				onConfirm={handleDiscardChanges}
			/>
		</>
	);
}
