import {
	type PointerEvent as ReactPointerEvent,
	type WheelEvent as ReactWheelEvent,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { formatBytes } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { FileInfo, FileListItem } from "@/types/api";
import {
	BlobImagePreview,
	type ImagePreviewSource,
	type ShowOriginalState,
} from "./BlobImagePreview";

const MIN_ZOOM = 0.5;
const MAX_ZOOM = 3;
const ZOOM_STEP = 0.25;
const IMAGE_TRANSFORM_ORIGIN = "center center";
const ORIGINAL_BUTTON_EXIT_MS = 220;

interface Point {
	x: number;
	y: number;
}

function clamp(value: number, min: number, max: number) {
	return Math.min(max, Math.max(min, value));
}

function distanceBetween(first: Point, second: Point) {
	return Math.hypot(first.x - second.x, first.y - second.y);
}

function midpoint(first: Point, second: Point): Point {
	return {
		x: (first.x + second.x) / 2,
		y: (first.y + second.y) / 2,
	};
}

interface ImagePreviewPanelProps {
	file: FileInfo | FileListItem;
	allOptionsCount: number;
	downloadPath: string;
	imagePreviewPath?: string;
	isExpanded: boolean;
	onChooseOpenMethod: () => void;
	onClose: () => void;
	onToggleExpand: () => void;
	chooseOpenMethodLabel: string;
	enterFullscreenLabel: string;
	exitFullscreenLabel: string;
	closeLabel: string;
	fitToWindowLabel: string;
	previewSourceLabel: string;
	originalSourceLabel: string;
	rotateRightLabel: string;
	zoomInLabel: string;
	zoomOutLabel: string;
}

export function ImagePreviewPanel({
	file,
	allOptionsCount,
	downloadPath,
	imagePreviewPath,
	isExpanded,
	onChooseOpenMethod,
	onClose,
	onToggleExpand,
	chooseOpenMethodLabel,
	enterFullscreenLabel,
	exitFullscreenLabel,
	closeLabel,
	fitToWindowLabel,
	previewSourceLabel,
	originalSourceLabel,
	rotateRightLabel,
	zoomInLabel,
	zoomOutLabel,
}: ImagePreviewPanelProps) {
	const imageRef = useRef<HTMLImageElement | null>(null);
	const viewportRef = useRef<HTMLDivElement | null>(null);
	const pointersRef = useRef(new Map<number, Point>());
	const dragStartRef = useRef<{
		imageOffset: Point;
		pointer: Point;
	} | null>(null);
	const pinchStartRef = useRef<{
		center: Point;
		distance: number;
		imageOffset: Point;
		zoom: number;
	} | null>(null);
	const [zoom, setZoom] = useState(1);
	const [rotation, setRotation] = useState(0);
	const [source, setSource] = useState<ImagePreviewSource>("original");
	const [imageOffset, setImageOffset] = useState<Point>({ x: 0, y: 0 });
	const [showOriginalState, setShowOriginalState] =
		useState<ShowOriginalState>("hidden");
	const [renderOriginalButton, setRenderOriginalButton] = useState(false);
	const [hideSuccessfulOriginalButton, setHideSuccessfulOriginalButton] =
		useState(false);
	const [showOriginalRequestId, setShowOriginalRequestId] = useState<
		number | undefined
	>();

	const sourceLabel =
		source === "backend_preview" ? previewSourceLabel : originalSourceLabel;
	const fullscreenLabel = isExpanded
		? exitFullscreenLabel
		: enterFullscreenLabel;
	const zoomPercent = Math.round(zoom * 100);
	const canZoomOut = zoom > MIN_ZOOM;
	const canZoomIn = zoom < MAX_ZOOM;

	const clampOffset = useCallback(
		(offset: Point, targetZoom: number): Point => {
			const image = imageRef.current;
			const viewport = viewportRef.current;
			if (!image || !viewport || targetZoom <= 1) {
				return { x: 0, y: 0 };
			}

			const viewportRect = viewport.getBoundingClientRect();
			const isSideways = rotation % 180 !== 0;
			const effectiveWidth = isSideways
				? image.offsetHeight
				: image.offsetWidth;
			const effectiveHeight = isSideways
				? image.offsetWidth
				: image.offsetHeight;
			const scaledWidth = effectiveWidth * targetZoom;
			const scaledHeight = effectiveHeight * targetZoom;
			const maxX = Math.max(0, (scaledWidth - viewportRect.width) / 2);
			const maxY = Math.max(0, (scaledHeight - viewportRect.height) / 2);

			return {
				x: clamp(offset.x, -maxX, maxX),
				y: clamp(offset.y, -maxY, maxY),
			};
		},
		[rotation],
	);

	const setClampedZoom = useCallback(
		(nextZoom: number, anchor?: Point) => {
			setZoom((currentZoom) => {
				const clampedZoom = clamp(nextZoom, MIN_ZOOM, MAX_ZOOM);
				setImageOffset((currentOffset) => {
					if (!anchor || currentZoom <= 0) {
						return clampOffset(currentOffset, clampedZoom);
					}

					const viewport = viewportRef.current;
					if (!viewport) {
						return clampOffset(currentOffset, clampedZoom);
					}

					const rect = viewport.getBoundingClientRect();
					const anchorFromCenter = {
						x: anchor.x - rect.left - rect.width / 2,
						y: anchor.y - rect.top - rect.height / 2,
					};
					const scaleDelta = clampedZoom / currentZoom;
					const anchoredOffset = {
						x:
							anchorFromCenter.x -
							(anchorFromCenter.x - currentOffset.x) * scaleDelta,
						y:
							anchorFromCenter.y -
							(anchorFromCenter.y - currentOffset.y) * scaleDelta,
					};
					return clampOffset(anchoredOffset, clampedZoom);
				});
				return clampedZoom;
			});
		},
		[clampOffset],
	);

	const resetImageTransform = useCallback(() => {
		setZoom(1);
		setImageOffset({ x: 0, y: 0 });
		setRotation(0);
	}, []);

	const zoomOut = useCallback(() => {
		setClampedZoom(zoom - ZOOM_STEP);
	}, [setClampedZoom, zoom]);

	const zoomIn = useCallback(() => {
		setClampedZoom(zoom + ZOOM_STEP);
	}, [setClampedZoom, zoom]);

	const rotateRight = useCallback(() => {
		setImageOffset({ x: 0, y: 0 });
		setRotation((current) => (current + 90) % 360);
	}, []);

	const showOriginal = useCallback(() => {
		setShowOriginalRequestId((current) => (current ?? 0) + 1);
	}, []);

	const imageStyle = useMemo(
		() => ({
			transform: `translate3d(${imageOffset.x}px, ${imageOffset.y}px, 0) scale(${zoom}) rotate(${rotation}deg)`,
			transformOrigin: IMAGE_TRANSFORM_ORIGIN,
			transition:
				pointersRef.current.size > 0 ? "none" : "transform 160ms ease-out",
		}),
		[imageOffset.x, imageOffset.y, rotation, zoom],
	);

	const handlePointerDown = useCallback(
		(event: ReactPointerEvent<HTMLDivElement>) => {
			pointersRef.current.set(event.pointerId, {
				x: event.clientX,
				y: event.clientY,
			});
			event.currentTarget.setPointerCapture(event.pointerId);

			const pointers = Array.from(pointersRef.current.values());
			if (pointers.length === 1) {
				dragStartRef.current = {
					imageOffset,
					pointer: pointers[0],
				};
				pinchStartRef.current = null;
				return;
			}

			if (pointers.length === 2) {
				dragStartRef.current = null;
				pinchStartRef.current = {
					center: midpoint(pointers[0], pointers[1]),
					distance: distanceBetween(pointers[0], pointers[1]),
					imageOffset,
					zoom,
				};
			}
		},
		[imageOffset, zoom],
	);

	const handlePointerMove = useCallback(
		(event: ReactPointerEvent<HTMLDivElement>) => {
			if (!pointersRef.current.has(event.pointerId)) return;
			pointersRef.current.set(event.pointerId, {
				x: event.clientX,
				y: event.clientY,
			});

			const pointers = Array.from(pointersRef.current.values());
			if (pointers.length === 2 && pinchStartRef.current) {
				event.preventDefault();
				const currentDistance = distanceBetween(pointers[0], pointers[1]);
				const currentCenter = midpoint(pointers[0], pointers[1]);
				if (pinchStartRef.current.distance <= 0) return;
				const nextZoom =
					pinchStartRef.current.zoom *
					(currentDistance / pinchStartRef.current.distance);
				const clampedZoom = clamp(nextZoom, MIN_ZOOM, MAX_ZOOM);
				const nextOffset = {
					x:
						pinchStartRef.current.imageOffset.x +
						currentCenter.x -
						pinchStartRef.current.center.x,
					y:
						pinchStartRef.current.imageOffset.y +
						currentCenter.y -
						pinchStartRef.current.center.y,
				};
				setZoom(clampedZoom);
				setImageOffset(clampOffset(nextOffset, clampedZoom));
				return;
			}

			if (pointers.length === 1 && dragStartRef.current && zoom > 1) {
				event.preventDefault();
				const nextOffset = {
					x:
						dragStartRef.current.imageOffset.x +
						pointers[0].x -
						dragStartRef.current.pointer.x,
					y:
						dragStartRef.current.imageOffset.y +
						pointers[0].y -
						dragStartRef.current.pointer.y,
				};
				setImageOffset(clampOffset(nextOffset, zoom));
			}
		},
		[clampOffset, zoom],
	);

	const handlePointerEnd = useCallback(
		(event: ReactPointerEvent<HTMLDivElement>) => {
			pointersRef.current.delete(event.pointerId);
			if (event.currentTarget.hasPointerCapture(event.pointerId)) {
				event.currentTarget.releasePointerCapture(event.pointerId);
			}

			const pointers = Array.from(pointersRef.current.values());
			if (pointers.length === 1) {
				dragStartRef.current = {
					imageOffset: clampOffset(imageOffset, zoom),
					pointer: pointers[0],
				};
				pinchStartRef.current = null;
				return;
			}

			dragStartRef.current = null;
			pinchStartRef.current = null;
			setImageOffset((current) => clampOffset(current, zoom));
		},
		[clampOffset, imageOffset, zoom],
	);

	const handleWheel = useCallback(
		(event: ReactWheelEvent<HTMLDivElement>) => {
			if (!event.ctrlKey && !event.metaKey) return;
			event.preventDefault();
			const direction = event.deltaY > 0 ? -1 : 1;
			setClampedZoom(zoom + direction * ZOOM_STEP, {
				x: event.clientX,
				y: event.clientY,
			});
		},
		[setClampedZoom, zoom],
	);

	useEffect(() => {
		const handleResize = () => {
			setImageOffset((current) => clampOffset(current, zoom));
		};
		window.addEventListener("resize", handleResize);
		return () => window.removeEventListener("resize", handleResize);
	}, [clampOffset, zoom]);

	useEffect(() => {
		if (showOriginalState === "success" && hideSuccessfulOriginalButton) {
			const timer = window.setTimeout(() => {
				setRenderOriginalButton(false);
			}, ORIGINAL_BUTTON_EXIT_MS);

			return () => {
				window.clearTimeout(timer);
			};
		}

		if (showOriginalState === "available" || showOriginalState === "loading") {
			setHideSuccessfulOriginalButton(false);
			setRenderOriginalButton(true);
			return;
		}

		if (showOriginalState === "success") {
			setRenderOriginalButton(true);
			const successTimer = window.setTimeout(() => {
				setHideSuccessfulOriginalButton(true);
			}, 650);
			return () => {
				window.clearTimeout(successTimer);
			};
		}

		const timer = window.setTimeout(() => {
			setRenderOriginalButton(false);
		}, ORIGINAL_BUTTON_EXIT_MS);

		return () => {
			window.clearTimeout(timer);
		};
	}, [hideSuccessfulOriginalButton, showOriginalState]);

	const originalButtonVisible =
		showOriginalState === "available" ||
		showOriginalState === "loading" ||
		(showOriginalState === "success" && !hideSuccessfulOriginalButton);
	const originalButtonDisabled =
		showOriginalState === "loading" || showOriginalState === "success";
	const originalButtonIcon =
		showOriginalState === "loading"
			? "Spinner"
			: showOriginalState === "success"
				? "Check"
				: "Eye";

	return (
		<div className="relative flex h-full min-h-0 flex-col overflow-hidden bg-zinc-950 text-white">
			<div className="absolute inset-x-0 top-0 z-10 bg-linear-to-b from-black/78 via-black/36 to-transparent px-3 pt-3 pb-8 opacity-0 transition-opacity duration-200 ease-out group-data-open/image-preview:opacity-100 group-data-closed/image-preview:opacity-0 sm:px-4">
				<div className="flex min-w-0 items-start gap-3">
					<div className="min-w-0 flex-1">
						<div className="flex min-w-0 items-center gap-2">
							<h2 className="min-w-0 truncate text-sm font-medium leading-6 text-white">
								{file.name}
							</h2>
							<span className="shrink-0 rounded-full border border-white/12 bg-white/10 px-2 py-0.5 text-[0.7rem] font-medium text-white/80">
								{sourceLabel}
							</span>
						</div>
						<p className="mt-0.5 truncate text-xs text-white/56">
							{formatBytes(file.size)}
							{file.mime_type ? ` · ${file.mime_type}` : ""}
						</p>
					</div>

					<div className="flex shrink-0 items-center gap-1 rounded-xl border border-white/10 bg-black/32 p-1 shadow-lg shadow-black/20 backdrop-blur-md">
						{allOptionsCount > 1 ? (
							<ToolbarButton
								label={chooseOpenMethodLabel}
								onClick={onChooseOpenMethod}
								icon="DotsThree"
							/>
						) : null}
						<ToolbarButton
							label={fullscreenLabel}
							onClick={onToggleExpand}
							icon={isExpanded ? "ArrowsInCardinal" : "ArrowsOutCardinal"}
						/>
						<ToolbarButton label={closeLabel} onClick={onClose} icon="X" />
					</div>
				</div>
			</div>

			<div className="min-h-0 flex-1 scale-[0.985] overflow-hidden opacity-0 transition-[opacity,transform] duration-200 ease-out group-data-open/image-preview:scale-100 group-data-open/image-preview:opacity-100 group-data-closed/image-preview:scale-[0.985] group-data-closed/image-preview:opacity-0">
				<div
					className={cn(
						"h-full min-h-0 w-full touch-none select-none overflow-hidden",
						zoom > 1 ? "cursor-grab active:cursor-grabbing" : "cursor-default",
					)}
					onPointerDown={handlePointerDown}
					onPointerMove={handlePointerMove}
					onPointerUp={handlePointerEnd}
					onPointerCancel={handlePointerEnd}
					onWheel={handleWheel}
				>
					<BlobImagePreview
						file={file}
						fillContainer
						path={downloadPath}
						fallbackPath={imagePreviewPath}
						imageRef={imageRef}
						viewportRef={viewportRef}
						onSourceChange={setSource}
						onShowOriginalStateChange={setShowOriginalState}
						showOriginalButtonPlacement="none"
						showOriginalRequestId={showOriginalRequestId}
						viewportClassName="flex h-full min-h-0 w-full items-center justify-center overflow-hidden px-4 py-16 sm:px-8"
						imageClassName="block max-h-full max-w-full min-w-0 touch-none select-none object-contain"
						imageStyle={imageStyle}
					/>
				</div>
			</div>

			<div className="pointer-events-none absolute inset-x-0 bottom-0 z-10 flex justify-center bg-linear-to-t from-black/72 via-black/28 to-transparent px-3 pt-10 pb-4 opacity-0 transition-opacity duration-200 ease-out group-data-open/image-preview:opacity-100 group-data-closed/image-preview:opacity-0">
				<div className="pointer-events-auto flex items-center gap-1 rounded-xl border border-white/10 bg-black/40 p-1 text-white shadow-lg shadow-black/25 backdrop-blur-md">
					{renderOriginalButton ? (
						<div
							className={cn(
								"flex origin-left items-center overflow-hidden transition-[max-width,opacity,transform] duration-[260ms] ease-out",
								originalButtonVisible
									? "max-w-28 translate-x-0 opacity-100"
									: "max-w-0 translate-x-2 opacity-0",
							)}
						>
							<div className="flex shrink-0 items-center gap-1">
								<Button
									type="button"
									variant="ghost"
									size="sm"
									className={cn(
										"text-white/82 shadow-none transition-[background-color,color,transform] duration-200 hover:bg-white/12 hover:text-white focus-visible:ring-white/35",
										originalButtonVisible ? "scale-100" : "scale-95",
										showOriginalState === "success" &&
											"bg-emerald-400/15 text-emerald-100 hover:bg-emerald-400/15 hover:text-emerald-100",
									)}
									onClick={showOriginal}
									disabled={originalButtonDisabled}
								>
									<Icon
										name={originalButtonIcon}
										className={cn(
											"size-4",
											showOriginalState === "loading" && "animate-spin",
										)}
									/>
									<span
										className={
											showOriginalState === "loading" ||
											showOriginalState === "success"
												? "sr-only"
												: undefined
										}
									>
										{originalSourceLabel}
									</span>
								</Button>
								<div className="mx-1 h-5 w-px bg-white/14" />
							</div>
						</div>
					) : null}
					<ToolbarButton
						label={zoomOutLabel}
						onClick={zoomOut}
						icon="MagnifyingGlassMinus"
						disabled={!canZoomOut}
					/>
					<button
						type="button"
						aria-label={fitToWindowLabel}
						className="h-8 min-w-15 rounded-lg px-2 text-xs font-medium text-white/82 transition-colors hover:bg-white/10 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/45"
						onClick={resetImageTransform}
						title={fitToWindowLabel}
					>
						{zoomPercent}%
					</button>
					<ToolbarButton
						label={zoomInLabel}
						onClick={zoomIn}
						icon="MagnifyingGlassPlus"
						disabled={!canZoomIn}
					/>
					<div className="mx-1 h-5 w-px bg-white/14" />
					<ToolbarButton
						label={rotateRightLabel}
						onClick={rotateRight}
						icon="ArrowClockwise"
					/>
				</div>
			</div>
		</div>
	);
}

function ToolbarButton({
	disabled,
	icon,
	label,
	onClick,
}: {
	disabled?: boolean;
	icon:
		| "ArrowClockwise"
		| "ArrowsInCardinal"
		| "ArrowsOutCardinal"
		| "DotsThree"
		| "MagnifyingGlassMinus"
		| "MagnifyingGlassPlus"
		| "X";
	label: string;
	onClick: () => void;
}) {
	return (
		<Button
			type="button"
			variant="ghost"
			size="icon-sm"
			disabled={disabled}
			onClick={onClick}
			aria-label={label}
			title={label}
			className="text-white/78 shadow-none hover:bg-white/12 hover:text-white focus-visible:ring-white/35 disabled:text-white/25"
		>
			<Icon name={icon} className="size-4" />
			<span className="sr-only">{label}</span>
		</Button>
	);
}
