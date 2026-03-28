import {
	type ComponentProps,
	type KeyboardEvent,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { useTranslation } from "react-i18next";
import { Document, Page, pdfjs } from "react-pdf";
import "react-pdf/dist/Page/AnnotationLayer.css";
import "react-pdf/dist/Page/TextLayer.css";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import { useBlobUrl } from "@/hooks/useBlobUrl";
import { PreviewError } from "./PreviewError";
import { PreviewLoadingState } from "./PreviewLoadingState";

pdfjs.GlobalWorkerOptions.workerSrc = new URL(
	"pdfjs-dist/build/pdf.worker.min.mjs",
	import.meta.url,
).toString();

const pdfDocumentOptions = {
	cMapUrl: `${import.meta.env.BASE_URL}pdfjs/${pdfjs.version}/cmaps/`,
	cMapPacked: true,
} satisfies NonNullable<ComponentProps<typeof Document>["options"]>;

const MIN_ZOOM = 50;
const MAX_ZOOM = 250;
const ZOOM_STEP = 25;
const VIEWER_HORIZONTAL_PADDING = 24;
const MIN_PAGE_WIDTH = 240;

type LoadedDocument = Parameters<
	NonNullable<ComponentProps<typeof Document>["onLoadSuccess"]>
>[0];
type LoadedPage = Parameters<
	NonNullable<ComponentProps<typeof Page>["onLoadSuccess"]>
>[0];

interface PdfPreviewProps {
	path: string;
	fileName?: string;
}

export function PdfPreview({ path, fileName }: PdfPreviewProps) {
	const { t } = useTranslation("files");
	const {
		blobUrl,
		error: blobError,
		loading: blobLoading,
		retry,
	} = useBlobUrl(path);
	const [numPages, setNumPages] = useState<number | null>(null);
	const [pdfError, setPdfError] = useState(false);
	const [currentPage, setCurrentPage] = useState(1);
	const [pageInputValue, setPageInputValue] = useState("1");
	const [zoomPercent, setZoomPercent] = useState(100);
	const [fitWidth, setFitWidth] = useState(true);
	const [rotation, setRotation] = useState(0);
	const [pageSize, setPageSize] = useState<{
		width: number;
		height: number;
	} | null>(null);
	const [viewerWidth, setViewerWidth] = useState(0);
	const pageRefs = useRef<Record<number, HTMLDivElement | null>>({});
	const scrollContainerRef = useRef<HTMLDivElement | null>(null);
	const scrollFrameRef = useRef<number | null>(null);

	const clampPageNumber = useCallback(
		(pageNumber: number) => {
			if (!numPages) return 1;
			return Math.min(Math.max(pageNumber, 1), numPages);
		},
		[numPages],
	);

	const clampZoom = useCallback((value: number) => {
		return Math.min(Math.max(value, MIN_ZOOM), MAX_ZOOM);
	}, []);

	const onDocumentLoadSuccess = useCallback(
		({ numPages: n }: LoadedDocument) => {
			setNumPages(n);
			setPdfError(false);
			setCurrentPage(1);
			setPageInputValue("1");
			pageRefs.current = {};
			if (scrollContainerRef.current) {
				scrollContainerRef.current.scrollTop = 0;
			}
		},
		[],
	);

	const onDocumentLoadError = useCallback(() => {
		setNumPages(null);
		setPdfError(true);
	}, []);

	const onPageLoadSuccess = useCallback((page: LoadedPage) => {
		setPageSize((currentSize) => {
			if (currentSize) return currentSize;
			const viewport = page.getViewport({ scale: 1 });
			return {
				width: viewport.width,
				height: viewport.height,
			};
		});
	}, []);

	const basePageWidth = useMemo(() => {
		if (!pageSize) return null;
		return rotation % 180 === 0 ? pageSize.width : pageSize.height;
	}, [pageSize, rotation]);

	const renderedPageWidth = useMemo(() => {
		if (fitWidth) {
			if (viewerWidth <= 0) return 800;
			return Math.max(
				Math.floor(viewerWidth - VIEWER_HORIZONTAL_PADDING),
				MIN_PAGE_WIDTH,
			);
		}
		if (!basePageWidth) {
			return 800;
		}
		return Math.max(
			Math.round((basePageWidth * clampZoom(zoomPercent)) / 100),
			MIN_PAGE_WIDTH,
		);
	}, [basePageWidth, clampZoom, fitWidth, viewerWidth, zoomPercent]);

	const effectiveZoomPercent = useMemo(() => {
		if (!basePageWidth) return clampZoom(zoomPercent);
		return clampZoom(Math.round((renderedPageWidth / basePageWidth) * 100));
	}, [basePageWidth, clampZoom, renderedPageWidth, zoomPercent]);
	const viewerLayoutVersion = `${renderedPageWidth}:${rotation}`;

	const syncCurrentPageFromScroll = useCallback(() => {
		const container = scrollContainerRef.current;
		if (!container || !numPages) return;

		const viewportMidpoint = container.scrollTop + container.clientHeight / 2;
		let closestPage = 1;
		let closestDistance = Number.POSITIVE_INFINITY;

		for (let pageNumber = 1; pageNumber <= numPages; pageNumber += 1) {
			const pageElement = pageRefs.current[pageNumber];
			if (!pageElement) continue;
			const pageMidpoint = pageElement.offsetTop + pageElement.offsetHeight / 2;
			const distance = Math.abs(pageMidpoint - viewportMidpoint);
			if (distance < closestDistance) {
				closestDistance = distance;
				closestPage = pageNumber;
			}
		}

		setCurrentPage((previousPage) =>
			previousPage === closestPage ? previousPage : closestPage,
		);
	}, [numPages]);

	const schedulePageSync = useCallback(() => {
		if (scrollFrameRef.current !== null) return;
		scrollFrameRef.current = window.requestAnimationFrame(() => {
			scrollFrameRef.current = null;
			syncCurrentPageFromScroll();
		});
	}, [syncCurrentPageFromScroll]);

	const scrollToPage = useCallback(
		(pageNumber: number, behavior: ScrollBehavior = "smooth") => {
			const container = scrollContainerRef.current;
			const pageElement = pageRefs.current[pageNumber];
			if (!container || !pageElement) return;
			container.scrollTo({
				top: Math.max(pageElement.offsetTop - 8, 0),
				behavior,
			});
			setCurrentPage(pageNumber);
			setPageInputValue(String(pageNumber));
		},
		[],
	);

	const commitPageInput = useCallback(() => {
		if (!numPages) {
			setPageInputValue("1");
			return;
		}
		const parsedPage = Number.parseInt(pageInputValue, 10);
		if (!Number.isFinite(parsedPage)) {
			setPageInputValue(String(currentPage));
			return;
		}
		scrollToPage(clampPageNumber(parsedPage));
	}, [clampPageNumber, currentPage, numPages, pageInputValue, scrollToPage]);

	const handlePageInputKeyDown = useCallback(
		(event: KeyboardEvent<HTMLInputElement>) => {
			if (event.key !== "Enter") return;
			event.preventDefault();
			commitPageInput();
		},
		[commitPageInput],
	);

	const setManualZoom = useCallback(
		(nextZoom: number) => {
			setFitWidth(false);
			setZoomPercent(clampZoom(nextZoom));
		},
		[clampZoom],
	);

	const handleZoomOut = useCallback(() => {
		setManualZoom(effectiveZoomPercent - ZOOM_STEP);
	}, [effectiveZoomPercent, setManualZoom]);

	const handleZoomIn = useCallback(() => {
		setManualZoom(effectiveZoomPercent + ZOOM_STEP);
	}, [effectiveZoomPercent, setManualZoom]);

	const handleResetZoom = useCallback(() => {
		setFitWidth(false);
		setZoomPercent(100);
	}, []);

	const handleRotateLeft = useCallback(() => {
		setRotation((currentRotation) => (currentRotation + 270) % 360);
	}, []);

	const handleRotateRight = useCallback(() => {
		setRotation((currentRotation) => (currentRotation + 90) % 360);
	}, []);

	const handleOpenInNewTab = useCallback(() => {
		if (!blobUrl) return;
		window.open(blobUrl, "_blank", "noopener,noreferrer");
	}, [blobUrl]);

	const handleDownload = useCallback(() => {
		if (!blobUrl) return;
		const link = document.createElement("a");
		link.href = blobUrl;
		link.download = fileName ?? "document.pdf";
		link.click();
	}, [blobUrl, fileName]);

	useEffect(() => {
		setNumPages(null);
		setPdfError(false);
		setCurrentPage(1);
		setPageInputValue("1");
		setZoomPercent(100);
		setFitWidth(true);
		setRotation(0);
		setPageSize(null);
		pageRefs.current = {};
		if (!blobUrl) {
			setViewerWidth(0);
			return;
		}
		if (scrollContainerRef.current) {
			setViewerWidth(scrollContainerRef.current.clientWidth);
			scrollContainerRef.current.scrollTop = 0;
		}
	}, [blobUrl]);

	useEffect(() => {
		if (!numPages) return;
		const container = scrollContainerRef.current;
		if (!container) return;

		const updateViewerWidth = () => {
			setViewerWidth(container.clientWidth);
		};

		updateViewerWidth();

		if (typeof ResizeObserver === "undefined") return;

		const resizeObserver = new ResizeObserver(() => {
			updateViewerWidth();
		});
		resizeObserver.observe(container);

		return () => {
			resizeObserver.disconnect();
		};
	}, [numPages]);

	useEffect(() => {
		setPageInputValue(String(currentPage));
	}, [currentPage]);

	useEffect(() => {
		if (!numPages) return;
		const frame = window.requestAnimationFrame(() => {
			void viewerLayoutVersion;
			syncCurrentPageFromScroll();
		});
		return () => {
			window.cancelAnimationFrame(frame);
		};
	}, [numPages, syncCurrentPageFromScroll, viewerLayoutVersion]);

	useEffect(() => {
		return () => {
			if (scrollFrameRef.current !== null) {
				window.cancelAnimationFrame(scrollFrameRef.current);
			}
		};
	}, []);

	if (blobLoading) {
		return (
			<PreviewLoadingState text={t("loading_preview")} className="h-full" />
		);
	}

	if (blobError || !blobUrl) {
		return <PreviewError onRetry={retry} />;
	}

	if (pdfError) {
		return <PreviewError onRetry={retry} />;
	}

	return (
		<div className="flex h-full min-h-0 w-full min-w-0 flex-col overflow-hidden rounded-xl border bg-background shadow-sm">
			<div className="border-b bg-muted/30 px-2.5 py-2">
				<div className="flex flex-wrap items-center gap-1.5 text-[11px] text-muted-foreground">
					<Icon name="FileText" className="h-3.5 w-3.5" />
					<span>PDF</span>
					{numPages !== null && (
						<>
							<span>·</span>
							<span className="tabular-nums">
								{t("pdf_page_count", { count: numPages })}
							</span>
							<span>·</span>
							<span className="tabular-nums">
								{t("pdf_page_of_total", { page: currentPage, count: numPages })}
							</span>
						</>
					)}
				</div>
				<div className="mt-2 flex flex-wrap items-center gap-1.5">
					<div className="flex items-center gap-1 rounded-lg border bg-background/80 p-0.5">
						<Button
							variant="ghost"
							size="icon-xs"
							onClick={() => scrollToPage(clampPageNumber(currentPage - 1))}
							disabled={numPages === null || currentPage <= 1}
							title={t("pdf_previous_page")}
							aria-label={t("pdf_previous_page")}
						>
							<Icon name="CaretLeft" className="h-4 w-4" />
						</Button>
						<Input
							value={pageInputValue}
							onChange={(event) => {
								const nextValue = event.target.value.replace(/\D+/g, "");
								setPageInputValue(nextValue);
							}}
							onBlur={commitPageInput}
							onKeyDown={handlePageInputKeyDown}
							inputMode="numeric"
							className="h-6 w-12 rounded-md px-1 text-center text-xs tabular-nums"
							aria-label={t("pdf_page_input_label")}
						/>
						<span className="min-w-[3rem] text-center text-[11px] text-muted-foreground tabular-nums">
							/ {numPages ?? "?"}
						</span>
						<Button
							variant="ghost"
							size="icon-xs"
							onClick={() => scrollToPage(clampPageNumber(currentPage + 1))}
							disabled={
								numPages === null ||
								(numPages !== null && currentPage >= numPages)
							}
							title={t("pdf_next_page")}
							aria-label={t("pdf_next_page")}
						>
							<Icon name="CaretRight" className="h-4 w-4" />
						</Button>
					</div>

					<div className="flex items-center gap-1 rounded-lg border bg-background/80 p-0.5">
						<Button
							variant="ghost"
							size="icon-xs"
							onClick={handleZoomOut}
							disabled={effectiveZoomPercent <= MIN_ZOOM}
							title={t("pdf_zoom_out")}
							aria-label={t("pdf_zoom_out")}
						>
							<Icon name="Minus" className="h-4 w-4" />
						</Button>
						<Button
							variant="ghost"
							size="xs"
							onClick={handleResetZoom}
							title={t("pdf_zoom_reset")}
							aria-label={t("pdf_zoom_reset")}
							className="min-w-[4rem] justify-center tabular-nums"
						>
							{t("pdf_zoom_percent", { zoom: effectiveZoomPercent })}
						</Button>
						<Button
							variant="ghost"
							size="icon-xs"
							onClick={handleZoomIn}
							disabled={effectiveZoomPercent >= MAX_ZOOM}
							title={t("pdf_zoom_in")}
							aria-label={t("pdf_zoom_in")}
						>
							<Icon name="Plus" className="h-4 w-4" />
						</Button>
						<Button
							variant={fitWidth ? "secondary" : "ghost"}
							size="xs"
							onClick={() => setFitWidth(true)}
						>
							{t("pdf_fit_width")}
						</Button>
					</div>

					<div className="flex items-center gap-1 rounded-lg border bg-background/80 p-0.5">
						<Button
							variant="ghost"
							size="icon-xs"
							onClick={handleRotateLeft}
							title={t("pdf_rotate_left")}
							aria-label={t("pdf_rotate_left")}
						>
							<Icon name="ArrowCounterClockwise" className="h-4 w-4" />
						</Button>
						<Button
							variant="ghost"
							size="icon-xs"
							onClick={handleRotateRight}
							title={t("pdf_rotate_right")}
							aria-label={t("pdf_rotate_right")}
						>
							<Icon name="ArrowClockwise" className="h-4 w-4" />
						</Button>
					</div>

					<div className="ml-auto flex items-center gap-1 rounded-lg border bg-background/80 p-0.5">
						<Button
							variant="ghost"
							size="icon-xs"
							onClick={handleOpenInNewTab}
							title={t("pdf_open_new_tab")}
							aria-label={t("pdf_open_new_tab")}
						>
							<Icon name="ArrowSquareOut" className="h-4 w-4" />
						</Button>
						<Button
							variant="ghost"
							size="icon-xs"
							onClick={handleDownload}
							title={t("pdf_download")}
							aria-label={t("pdf_download")}
						>
							<Icon name="Download" className="h-4 w-4" />
						</Button>
					</div>
				</div>
			</div>
			<div
				ref={scrollContainerRef}
				onScroll={schedulePageSync}
				className="min-h-0 flex-1 overflow-auto bg-muted/20 px-3 py-3"
			>
				<Document
					file={blobUrl}
					options={pdfDocumentOptions}
					onLoadSuccess={onDocumentLoadSuccess}
					onLoadError={onDocumentLoadError}
					loading={
						<div className="p-6 text-sm text-muted-foreground">
							{t("loading_preview")}
						</div>
					}
				>
					<div className="space-y-3">
						{numPages !== null &&
							Array.from({ length: numPages }, (_, index) => {
								const pageNumber = index + 1;
								return (
									<div
										key={`page_${pageNumber}`}
										ref={(node) => {
											pageRefs.current[pageNumber] = node;
										}}
										className="mx-auto flex justify-center"
									>
										<div className="overflow-hidden rounded-lg bg-white shadow-sm ring-1 ring-black/5">
											<Page
												pageNumber={pageNumber}
												width={renderedPageWidth}
												rotate={rotation}
												onLoadSuccess={onPageLoadSuccess}
												loading={
													<div className="flex h-[250px] w-[200px] items-center justify-center bg-white">
														<span className="text-sm text-muted-foreground">
															{t("loading_preview")}
														</span>
													</div>
												}
											/>
										</div>
									</div>
								);
							})}
					</div>
				</Document>
			</div>
		</div>
	);
}
