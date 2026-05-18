import type { ChangeEvent, CSSProperties, ReactNode } from "react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Icon } from "@/components/ui/icon";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import { resolveApiResourceUrl } from "@/lib/apiUrl";
import { formatBytes } from "@/lib/format";
import { logger } from "@/lib/logger";
import { parseMusicMetadataFromSource } from "@/lib/musicPlayer";
import { cn } from "@/lib/utils";
import {
	type MusicPlaybackMode,
	type MusicPlayerTrack,
	useMusicPlayerStore,
} from "@/stores/musicPlayerStore";

const STREAM_REFRESH_LEAD_MS = 2 * 60 * 1000;
const STREAM_REFRESH_MIN_DELAY_MS = 10 * 1000;

function formatPlaybackTime(seconds: number) {
	if (!Number.isFinite(seconds) || seconds < 0) {
		return "0:00";
	}

	const totalSeconds = Math.floor(seconds);
	const minutes = Math.floor(totalSeconds / 60);
	const remainingSeconds = totalSeconds % 60;
	return `${minutes}:${remainingSeconds.toString().padStart(2, "0")}`;
}

function sessionRefreshDelay(expiresAt?: string) {
	if (!expiresAt) return null;

	const expiresAtMs = new Date(expiresAt).getTime();
	if (!Number.isFinite(expiresAtMs)) return null;

	return Math.max(
		STREAM_REFRESH_MIN_DELAY_MS,
		expiresAtMs - Date.now() - STREAM_REFRESH_LEAD_MS,
	);
}

function displayTitle(track: MusicPlayerTrack) {
	return track.metadata?.title || track.name;
}

function displayArtist(track: MusicPlayerTrack) {
	if (track.metadata?.artists && track.metadata.artists.length > 0) {
		return track.metadata.artists.join(", ");
	}
	return track.metadata?.artist || null;
}

const MUSIC_TEXT_MARQUEE_KEYFRAMES = `
@keyframes music-player-text-marquee {
	0% {
		transform: translateX(0);
	}
	12% {
		transform: translateX(0);
	}
	82% {
		transform: translateX(var(--music-text-scroll-distance));
	}
	100% {
		transform: translateX(var(--music-text-scroll-distance));
	}
}
`;
const MUSIC_TEXT_MARQUEE_COPY_GAP_PX = 24;

function useAutoScrollState(text: string, enabled: boolean) {
	const viewportRef = useRef<HTMLDivElement | null>(null);
	const trackRef = useRef<HTMLDivElement | HTMLSpanElement | null>(null);
	const [isOverflowing, setIsOverflowing] = useState(false);
	const [scrollDistance, setScrollDistance] = useState(0);

	useLayoutEffect(() => {
		void text;
		if (!enabled) {
			setIsOverflowing(false);
			setScrollDistance(0);
			return;
		}

		const measure = () => {
			const viewport = viewportRef.current;
			const track = trackRef.current;
			if (!viewport || !track) return;
			const overflowDistance = Math.max(
				0,
				track.scrollWidth - viewport.clientWidth,
			);
			setScrollDistance(track.scrollWidth + MUSIC_TEXT_MARQUEE_COPY_GAP_PX);
			setIsOverflowing(overflowDistance > 1);
		};

		measure();

		const viewport = viewportRef.current;
		const ro =
			typeof ResizeObserver === "undefined"
				? null
				: new ResizeObserver(() => {
						measure();
					});

		if (viewport) {
			ro?.observe(viewport);
		}
		if (trackRef.current) {
			ro?.observe(trackRef.current);
		}

		const raf = window.requestAnimationFrame(measure);
		window.addEventListener("resize", measure);

		return () => {
			window.cancelAnimationFrame(raf);
			window.removeEventListener("resize", measure);
			ro?.disconnect();
		};
	}, [enabled, text]);

	return { isOverflowing, scrollDistance, trackRef, viewportRef };
}

function AutoScrollText({
	active,
	children,
	className,
}: {
	active: boolean;
	children: string;
	className?: string;
}) {
	const { isOverflowing, scrollDistance, trackRef, viewportRef } =
		useAutoScrollState(children, active);
	const shouldMarquee = active && isOverflowing;
	const animationDuration = Math.min(28, Math.max(12, 8 + scrollDistance / 36));

	return (
		<div
			ref={viewportRef}
			className={cn("min-w-0 overflow-hidden", "select-text", className)}
			data-marquee-active={String(shouldMarquee)}
		>
			{shouldMarquee ? (
				<>
					<style>{MUSIC_TEXT_MARQUEE_KEYFRAMES}</style>
					<span
						className={cn(
							"flex w-max max-w-none gap-6 whitespace-nowrap will-change-transform hover:[animation-play-state:paused] motion-reduce:[animation:none]",
						)}
						style={
							{
								animation: `music-player-text-marquee ${animationDuration}s linear infinite`,
								"--music-text-scroll-distance": `-${scrollDistance}px`,
							} as CSSProperties
						}
					>
						<span
							ref={(node) => {
								trackRef.current = node;
							}}
							className="shrink-0"
						>
							{children}
						</span>
						<span className="shrink-0" aria-hidden="true">
							{children}
						</span>
					</span>
				</>
			) : (
				<span
					ref={(node) => {
						trackRef.current = node;
					}}
					className="block min-w-0 truncate whitespace-nowrap"
				>
					{children}
				</span>
			)}
		</div>
	);
}

function playbackModeIcon(mode: MusicPlaybackMode) {
	if (mode === "shuffle") return "Shuffle";
	if (mode === "repeat_one") return "RepeatOnce";
	return "Repeat";
}

function nextPlaybackMode(mode: MusicPlaybackMode): MusicPlaybackMode {
	if (mode === "repeat_queue") return "repeat_one";
	if (mode === "repeat_one") return "shuffle";
	return "repeat_queue";
}

function MusicArtwork({
	className,
	track,
}: {
	className?: string;
	track: MusicPlayerTrack | null;
}) {
	if (track?.metadata?.artworkUrl) {
		return (
			<img
				src={track.metadata.artworkUrl}
				alt=""
				className={cn("object-cover", className)}
			/>
		);
	}

	return (
		<div
			className={cn(
				"flex items-center justify-center overflow-hidden rounded-lg border border-border/55 bg-[linear-gradient(135deg,var(--color-muted),var(--color-background))] text-primary",
				className,
			)}
		>
			<Icon name="VinylRecord" className="h-1/2 w-1/2 opacity-80" />
		</div>
	);
}

function PlayerIconButton({
	active = false,
	children,
	label,
	onClick,
}: {
	active?: boolean;
	children: ReactNode;
	label: string;
	onClick: () => void;
}) {
	return (
		<Tooltip>
			<TooltipTrigger
				render={
					<Button
						type="button"
						variant={active ? "secondary" : "ghost"}
						size="icon-sm"
						onClick={onClick}
						aria-label={label}
					/>
				}
			>
				{children}
			</TooltipTrigger>
			<TooltipContent>{label}</TooltipContent>
		</Tooltip>
	);
}

export function MusicPlayerHost() {
	const { t } = useTranslation("files");
	const audioRef = useRef<HTMLAudioElement | null>(null);
	const isSeekingRef = useRef(false);
	const parsedMetadataTrackIdsRef = useRef(new Set<string>());
	const wasPlayingBeforeSeekRef = useRef(false);
	const [currentTime, setCurrentTime] = useState(0);
	const [duration, setDuration] = useState(0);
	const [volume, setVolume] = useState(0.85);
	const activeTrackId = useMusicPlayerStore((state) => state.activeTrackId);
	const error = useMusicPlayerStore((state) => state.error);
	const isPanelOpen = useMusicPlayerStore((state) => state.isPanelOpen);
	const isPlaying = useMusicPlayerStore((state) => state.isPlaying);
	const playRequested = useMusicPlayerStore((state) => state.playRequested);
	const playRequestVersion = useMusicPlayerStore(
		(state) => state.playRequestVersion,
	);
	const playbackMode = useMusicPlayerStore((state) => state.playbackMode);
	const queue = useMusicPlayerStore((state) => state.queue);
	const closePanel = useMusicPlayerStore((state) => state.closePanel);
	const clear = useMusicPlayerStore((state) => state.clear);
	const playNext = useMusicPlayerStore((state) => state.playNext);
	const playPrevious = useMusicPlayerStore((state) => state.playPrevious);
	const playTracks = useMusicPlayerStore((state) => state.playTracks);
	const requestPlayback = useMusicPlayerStore((state) => state.requestPlayback);
	const setError = useMusicPlayerStore((state) => state.setError);
	const setPanelOpen = useMusicPlayerStore((state) => state.setPanelOpen);
	const setPlaybackMode = useMusicPlayerStore((state) => state.setPlaybackMode);
	const setPlaying = useMusicPlayerStore((state) => state.setPlaying);
	const setPlaybackRequested = useMusicPlayerStore(
		(state) => state.setPlaybackRequested,
	);
	const updateTrackMetadata = useMusicPlayerStore(
		(state) => state.updateTrackMetadata,
	);
	const updateTrackSource = useMusicPlayerStore(
		(state) => state.updateTrackSource,
	);
	const track = useMemo(
		() => queue.find((candidate) => candidate.id === activeTrackId) ?? null,
		[activeTrackId, queue],
	);
	const source = useMemo(
		() => (track ? resolveApiResourceUrl(track.path) : null),
		[track],
	);
	const trackKey = track ? `${track.id}:${track.path}` : null;
	const progress =
		duration > 0 && Number.isFinite(duration)
			? Math.min(100, Math.max(0, (currentTime / duration) * 100))
			: 0;
	const modeLabel = t(`music_player_mode_${playbackMode}`);

	useEffect(() => {
		if (!trackKey) return;
		setCurrentTime(0);
		setDuration(0);
	}, [trackKey]);

	useEffect(() => {
		if (!track || !source || parsedMetadataTrackIdsRef.current.has(track.id)) {
			return;
		}
		parsedMetadataTrackIdsRef.current.add(track.id);

		const controller = new AbortController();
		const trackId = track.id;
		const fallbackMetadata = track.metadata;
		const mimeType = track.mimeType;
		const name = track.name;
		const size = track.size;
		void parseMusicMetadataFromSource({
			fallbackMetadata,
			mimeType,
			name,
			signal: controller.signal,
			size,
			source,
		})
			.then((metadata) => {
				updateTrackMetadata(trackId, metadata);
			})
			.catch((metadataError) => {
				if (controller.signal.aborted) return;
				logger.debug("music metadata parse failed", name, metadataError);
			});

		return () => {
			controller.abort();
		};
	}, [source, track, updateTrackMetadata]);

	useEffect(() => {
		const audio = audioRef.current;
		if (!audio) return;
		audio.volume = volume;
	}, [volume]);

	useEffect(() => {
		if (!track?.refreshStreamLink) return;

		const delay = sessionRefreshDelay(track.expiresAt);
		if (delay === null) return;

		const timer = window.setTimeout(() => {
			track
				.refreshStreamLink?.()
				.then((link) => {
					updateTrackSource(track.id, link);
				})
				.catch((refreshError) => {
					logger.warn(
						"music stream session refresh failed",
						track.name,
						refreshError,
					);
				});
		}, delay);

		return () => window.clearTimeout(timer);
	}, [track, updateTrackSource]);

	useEffect(() => {
		const audio = audioRef.current;
		if (!audio || !source) return;
		void playRequestVersion;

		if (!playRequested) {
			audio.pause();
			return;
		}

		void audio.play().catch((playError) => {
			logger.warn("music playback start failed", track?.name, playError);
			setPlaybackRequested(false);
			setPlaying(false);
		});
	}, [
		playRequestVersion,
		playRequested,
		setPlaybackRequested,
		setPlaying,
		source,
		track?.name,
	]);

	if (!track || !source) {
		return null;
	}

	const togglePlayback = () => {
		if (isPlaying) {
			audioRef.current?.pause();
			setPlaybackRequested(false);
			return;
		}

		requestPlayback();
	};

	const handleSeek = (event: ChangeEvent<HTMLInputElement>) => {
		const audio = audioRef.current;
		if (!audio || duration <= 0) return;

		const nextTime = (Number(event.currentTarget.value) / 100) * duration;
		audio.currentTime = nextTime;
		setCurrentTime(nextTime);
	};

	const beginSeek = () => {
		if (isSeekingRef.current) return;
		isSeekingRef.current = true;
		wasPlayingBeforeSeekRef.current = isPlaying || playRequested;

		if (wasPlayingBeforeSeekRef.current) {
			audioRef.current?.pause();
			setPlaybackRequested(false);
		}
	};

	const endSeek = () => {
		if (!isSeekingRef.current) return;
		isSeekingRef.current = false;

		if (wasPlayingBeforeSeekRef.current) {
			requestPlayback();
		}
		wasPlayingBeforeSeekRef.current = false;
	};

	const handleVolumeChange = (event: ChangeEvent<HTMLInputElement>) => {
		const nextVolume = Number(event.currentTarget.value) / 100;
		if (!Number.isFinite(nextVolume)) return;
		setVolume(Math.min(1, Math.max(0, nextVolume)));
	};

	const activateQueueTrack = (trackId: string) => {
		playTracks(queue, trackId);
	};

	return (
		<>
			{/* biome-ignore lint/a11y/useMediaCaption: user-uploaded media may not have captions available */}
			<audio
				ref={audioRef}
				src={source ?? undefined}
				preload="metadata"
				onCanPlay={() => setError(null)}
				onDurationChange={(event) =>
					setDuration(event.currentTarget.duration || 0)
				}
				onEnded={() => {
					if (playbackMode === "repeat_one") {
						const audio = audioRef.current;
						if (audio) {
							audio.currentTime = 0;
						}
						requestPlayback();
						return;
					}
					playNext();
				}}
				onError={() => {
					setError(t("music_player_load_failed"));
					setPlaybackRequested(false);
					setPlaying(false);
				}}
				onLoadedMetadata={(event) => {
					setDuration(event.currentTarget.duration || 0);
				}}
				onPause={() => setPlaying(false)}
				onPlay={() => {
					setError(null);
					setPlaying(true);
					setPlaybackRequested(true);
				}}
				onTimeUpdate={(event) =>
					setCurrentTime(event.currentTarget.currentTime || 0)
				}
			/>

			<Dialog open={isPanelOpen} onOpenChange={setPanelOpen}>
				<DialogContent
					showCloseButton={false}
					className="top-auto right-3 bottom-16 left-auto flex h-[min(42rem,calc(100vh-6rem))] w-[calc(100vw-1.5rem)] max-w-[26rem] translate-x-0 translate-y-0 grid-cols-none flex-col gap-0 overflow-hidden rounded-lg p-0 sm:right-4 sm:bottom-20 sm:w-[26rem]"
				>
					<DialogHeader className="border-b border-border/65 px-4 py-3">
						<div className="flex items-center justify-between gap-3">
							<DialogTitle className="flex min-w-0 items-center gap-2">
								<Icon name="MusicNotes" className="h-4 w-4 text-primary" />
								<span className="truncate">{t("music_player_title")}</span>
							</DialogTitle>
							<div className="flex items-center gap-1">
								<TooltipProvider>
									<PlayerIconButton
										label={t("music_player_close")}
										onClick={clear}
									>
										<Icon name="X" className="h-4 w-4" />
									</PlayerIconButton>
									<PlayerIconButton
										label={t("music_player_collapse")}
										onClick={closePanel}
									>
										<Icon name="CaretDown" className="h-4 w-4" />
									</PlayerIconButton>
								</TooltipProvider>
							</div>
						</div>
					</DialogHeader>

					<div className="flex min-h-0 flex-1 flex-col">
						<div className="px-4 pt-4">
							<div className="flex min-w-0 gap-3">
								<MusicArtwork
									track={track}
									className="h-24 w-24 shrink-0 rounded-lg"
								/>
								<div className="flex min-w-0 flex-1 flex-col justify-center">
									<AutoScrollText
										active
										className="text-base font-semibold leading-6"
									>
										{displayTitle(track)}
									</AutoScrollText>
									<AutoScrollText
										active
										className="mt-1 text-sm text-muted-foreground"
									>
										{displayArtist(track) ?? t("music_player_unknown_artist")}
									</AutoScrollText>
									<div className="mt-2 flex min-w-0 flex-wrap items-center gap-1.5">
										<Badge variant="outline">
											{formatPlaybackTime(duration)}
										</Badge>
										{track.size !== undefined ? (
											<Badge variant="outline">{formatBytes(track.size)}</Badge>
										) : null}
									</div>
								</div>
							</div>

							<div className="mt-4 space-y-2">
								<input
									type="range"
									min={0}
									max={100}
									step={0.1}
									value={progress}
									onChange={handleSeek}
									onBlur={endSeek}
									onKeyDown={beginSeek}
									onKeyUp={endSeek}
									onPointerCancel={endSeek}
									onPointerDown={beginSeek}
									onPointerUp={endSeek}
									aria-label={t("music_player_seek")}
									className={cn(
										"h-2 w-full cursor-pointer appearance-none rounded-full bg-muted accent-primary",
										duration <= 0 && "cursor-default opacity-60",
									)}
									disabled={duration <= 0}
								/>
								<div className="flex items-center justify-between text-[11px] tabular-nums text-muted-foreground">
									<span>{formatPlaybackTime(currentTime)}</span>
									<span>{formatPlaybackTime(duration)}</span>
								</div>
							</div>

							<TooltipProvider>
								<div className="mt-3 flex items-center justify-center gap-1">
									<PlayerIconButton
										active={playbackMode !== "repeat_queue"}
										label={modeLabel}
										onClick={() =>
											setPlaybackMode(nextPlaybackMode(playbackMode))
										}
									>
										<Icon
											name={playbackModeIcon(playbackMode)}
											className="h-4 w-4"
										/>
									</PlayerIconButton>
									<PlayerIconButton
										label={t("music_player_previous")}
										onClick={playPrevious}
									>
										<Icon name="SkipBack" className="h-4 w-4" />
									</PlayerIconButton>
									<Button
										type="button"
										variant="default"
										size="icon"
										className="mx-1 h-11 w-11 rounded-full"
										onClick={togglePlayback}
										aria-label={
											isPlaying
												? t("music_player_pause")
												: t("music_player_play")
										}
									>
										<Icon
											name={isPlaying ? "Pause" : "Play"}
											className="h-5 w-5"
										/>
									</Button>
									<PlayerIconButton
										label={t("music_player_next")}
										onClick={playNext}
									>
										<Icon name="SkipForward" className="h-4 w-4" />
									</PlayerIconButton>
									<div className="flex h-8 items-center gap-1 rounded-md px-1">
										<Icon
											name={volume === 0 ? "SpeakerSlash" : "SpeakerHigh"}
											className="h-4 w-4 text-muted-foreground"
										/>
										<input
											type="range"
											min={0}
											max={100}
											step={1}
											value={Math.round(volume * 100)}
											onChange={handleVolumeChange}
											aria-label={t("music_player_volume")}
											className="h-1.5 w-16 cursor-pointer appearance-none rounded-full bg-muted accent-primary"
										/>
									</div>
								</div>
							</TooltipProvider>

							{error ? (
								<p className="mt-3 rounded-md border border-destructive/25 bg-destructive/8 px-3 py-2 text-xs text-destructive">
									{error}
								</p>
							) : null}
						</div>

						<Tabs defaultValue="queue" className="mt-4 min-h-0 flex-1 gap-0">
							<TabsList variant="line" className="px-4">
								<TabsTrigger value="queue">
									<Icon name="Queue" className="h-4 w-4" />
									{t("music_player_queue")}
								</TabsTrigger>
								<TabsTrigger value="details">
									<Icon name="Info" className="h-4 w-4" />
									{t("music_player_details")}
								</TabsTrigger>
							</TabsList>
							<TabsContent
								value="queue"
								className="min-h-0 border-t border-border/65"
							>
								<ScrollArea className="h-full">
									<div className="space-y-1 p-2">
										{queue.map((queueTrack, index) => {
											const active = queueTrack.id === activeTrackId;
											return (
												<button
													key={queueTrack.id}
													type="button"
													className={cn(
														"flex w-full min-w-0 items-center gap-3 rounded-md px-2 py-2 text-left transition hover:bg-muted/55",
														active &&
															"bg-primary/10 text-primary hover:bg-primary/12",
													)}
													onClick={() => activateQueueTrack(queueTrack.id)}
												>
													<div
														className={cn(
															"flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-muted text-xs tabular-nums text-muted-foreground",
															active && "bg-primary/15 text-primary",
														)}
													>
														{active && isPlaying ? (
															<Icon name="MusicNotes" className="h-4 w-4" />
														) : (
															index + 1
														)}
													</div>
													<div className="min-w-0 flex-1">
														{active ? (
															<AutoScrollText
																active
																className="text-sm font-medium"
															>
																{displayTitle(queueTrack)}
															</AutoScrollText>
														) : (
															<span className="block truncate whitespace-nowrap text-sm font-medium">
																{displayTitle(queueTrack)}
															</span>
														)}
														{active ? (
															<AutoScrollText
																active
																className="text-xs text-muted-foreground"
															>
																{displayArtist(queueTrack) ??
																	t("music_player_unknown_artist")}
															</AutoScrollText>
														) : (
															<span className="block truncate whitespace-nowrap text-xs text-muted-foreground">
																{displayArtist(queueTrack) ??
																	t("music_player_unknown_artist")}
															</span>
														)}
													</div>
												</button>
											);
										})}
									</div>
								</ScrollArea>
							</TabsContent>
							<TabsContent
								value="details"
								className="min-h-0 border-t border-border/65"
							>
								<div className="space-y-3 p-4 text-sm">
									<div>
										<div className="text-xs font-medium uppercase text-muted-foreground">
											{t("music_player_file_name")}
										</div>
										<AutoScrollText active className="mt-1">
											{track.name}
										</AutoScrollText>
									</div>
									<div>
										<div className="text-xs font-medium uppercase text-muted-foreground">
											{t("music_player_mime_type")}
										</div>
										<AutoScrollText active className="mt-1">
											{track.mimeType}
										</AutoScrollText>
									</div>
									<div>
										<div className="text-xs font-medium uppercase text-muted-foreground">
											{t("music_player_mode")}
										</div>
										<p className="mt-1">{modeLabel}</p>
									</div>
								</div>
							</TabsContent>
						</Tabs>
					</div>
				</DialogContent>
			</Dialog>
		</>
	);
}
