import type React from "react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useLocation, useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { SkeletonTree } from "@/components/common/SkeletonTree";
import { Icon } from "@/components/ui/icon";
import { handleApiError } from "@/hooks/useApiError";
import { DRAG_MIME } from "@/lib/constants";
import { formatBatchToast } from "@/lib/formatBatchToast";
import { cn } from "@/lib/utils";
import { fileService } from "@/services/fileService";
import { useAuthStore } from "@/stores/authStore";
import { useFileStore } from "@/stores/fileStore";
import type { FolderInfo } from "@/types/api";

interface FolderTreeNode {
	folder: FolderInfo;
	parentId: number | null;
	childIds: number[];
}

interface FolderTreeSnapshot {
	expandedIds: number[];
	loadedIds: number[];
	nodeEntries: Array<[number, FolderTreeNode]>;
	rootIds: number[];
	userId: number | null;
}

interface TreeNodeProps {
	currentFolderId: number | null;
	depth: number;
	expandedIds: Set<number>;
	loadedIds: Set<number>;
	loadingIds: Set<number>;
	nodeId: number;
	nodeMap: Map<number, FolderTreeNode>;
	onDrop: (
		fileIds: number[],
		folderIds: number[],
		targetFolderId: number,
	) => void;
	onNavigate: (id: number, name: string) => void;
	onToggle: (id: number) => void;
	renderChildren: (ids: number[], depth: number) => React.ReactNode;
}

let folderTreeSnapshot: FolderTreeSnapshot | null = null;

function cloneNodeEntries(
	nodeMap: Map<number, FolderTreeNode>,
): Array<[number, FolderTreeNode]> {
	return Array.from(nodeMap.entries()).map(([id, node]) => [
		id,
		{
			folder: node.folder,
			parentId: node.parentId,
			childIds: [...node.childIds],
		},
	]);
}

function upsertChildren(
	nodeMap: Map<number, FolderTreeNode>,
	parentId: number | null,
	folders: FolderInfo[],
): { nodeMap: Map<number, FolderTreeNode>; rootIds: number[] } {
	const nextNodeMap = new Map(nodeMap);
	const childIds = folders.map((folder) => folder.id);

	for (const folder of folders) {
		const existing = nextNodeMap.get(folder.id);
		nextNodeMap.set(folder.id, {
			childIds: existing?.childIds ?? [],
			folder,
			parentId,
		});
	}

	if (parentId === null) {
		return { nodeMap: nextNodeMap, rootIds: childIds };
	}

	const parentNode = nextNodeMap.get(parentId);
	if (parentNode) {
		nextNodeMap.set(parentId, {
			...parentNode,
			childIds,
		});
	}

	return { nodeMap: nextNodeMap, rootIds: [] };
}

function TreeNode({
	currentFolderId,
	depth,
	expandedIds,
	loadedIds,
	loadingIds,
	nodeId,
	nodeMap,
	onDrop,
	onNavigate,
	onToggle,
	renderChildren,
}: TreeNodeProps) {
	const node = nodeMap.get(nodeId);
	const [dragOver, setDragOver] = useState(false);

	if (!node) return null;

	const isActive = currentFolderId === node.folder.id;
	const isExpanded = expandedIds.has(node.folder.id);
	const isLoading = loadingIds.has(node.folder.id);
	const isLoaded = loadedIds.has(node.folder.id);
	const showToggle = isLoading || !isLoaded || node.childIds.length > 0;

	const handleDragOver = (e: React.DragEvent) => {
		if (!e.dataTransfer.types.includes(DRAG_MIME)) return;
		e.preventDefault();
		e.dataTransfer.dropEffect = "move";
		setDragOver(true);
	};

	const handleDragLeave = () => {
		setDragOver(false);
	};

	const handleDrop = (e: React.DragEvent) => {
		setDragOver(false);
		e.preventDefault();
		const raw = e.dataTransfer.getData(DRAG_MIME);
		if (!raw) return;
		const data = JSON.parse(raw) as { fileIds: number[]; folderIds: number[] };
		if (data.folderIds.includes(node.folder.id)) return;
		onDrop(data.fileIds, data.folderIds, node.folder.id);
	};

	return (
		<div>
			{/* biome-ignore lint/a11y/noStaticElementInteractions: drag-drop target */}
			<div
				className={cn(
					"flex items-center gap-0.5 py-1.5 rounded-md text-sm hover:bg-accent transition-colors",
					isActive && "bg-accent font-medium",
					dragOver && "ring-2 ring-primary bg-accent/30",
				)}
				style={{ paddingLeft: `${depth * 16 + 4}px` }}
				onDragOver={handleDragOver}
				onDragLeave={handleDragLeave}
				onDrop={handleDrop}
			>
				{showToggle ? (
					<button
						type="button"
						className="p-0.5 hover:bg-accent-foreground/10 rounded shrink-0 disabled:cursor-default disabled:hover:bg-transparent"
						onClick={() => onToggle(node.folder.id)}
						disabled={isLoading}
					>
						{isLoading ? (
							<div className="h-3 w-3 border-2 border-muted-foreground/30 border-t-muted-foreground rounded-full animate-spin" />
						) : isExpanded ? (
							<Icon
								name="CaretDown"
								className="h-3 w-3 text-muted-foreground"
							/>
						) : (
							<Icon
								name="CaretRight"
								className="h-3 w-3 text-muted-foreground"
							/>
						)}
					</button>
				) : (
					<span className="h-4 w-4 shrink-0" aria-hidden="true" />
				)}
				<button
					type="button"
					className="flex-1 flex items-center gap-1.5 text-left min-w-0 px-1"
					onClick={() => onNavigate(node.folder.id, node.folder.name)}
				>
					{isExpanded ? (
						<Icon
							name="FolderOpen"
							className="h-4 w-4 text-muted-foreground shrink-0"
						/>
					) : (
						<Icon
							name="Folder"
							className="h-4 w-4 text-muted-foreground shrink-0"
						/>
					)}
					<span className="truncate">{node.folder.name}</span>
				</button>
			</div>
			{isExpanded && node.childIds.length > 0 && renderChildren(node.childIds, depth + 1)}
		</div>
	);
}

export function FolderTree() {
	const { t } = useTranslation("files");
	const userId = useAuthStore((s) => s.user?.id ?? null);
	const location = useLocation();
	const navigate = useNavigate();
	const breadcrumb = useFileStore((s) => s.breadcrumb);
	const currentFolderId = useFileStore((s) => s.currentFolderId);
	const moveToFolder = useFileStore((s) => s.moveToFolder);
	const storeFolders = useFileStore((s) => s.folders);
	const storeCurrentFolderId = useFileStore((s) => s.currentFolderId);
	const storeLoading = useFileStore((s) => s.loading);
	const isRootRoute = location.pathname === "/";
	const cachedSnapshot = folderTreeSnapshot?.userId === userId ? folderTreeSnapshot : null;

	const [nodeMap, setNodeMap] = useState<Map<number, FolderTreeNode>>(
		() => new Map(cachedSnapshot?.nodeEntries ?? []),
	);
	const [rootIds, setRootIds] = useState<number[]>(() => cachedSnapshot?.rootIds ?? []);
	const [expandedIds, setExpandedIds] = useState<Set<number>>(
		() => new Set(cachedSnapshot?.expandedIds ?? []),
	);
	const [loadingIds, setLoadingIds] = useState<Set<number>>(() => new Set());
	const [loadedIds, setLoadedIds] = useState<Set<number>>(
		() => new Set(cachedSnapshot?.loadedIds ?? []),
	);
	const [rootLoaded, setRootLoaded] = useState(
		() => cachedSnapshot !== null || rootIds.length > 0,
	);
	const [rootDragOver, setRootDragOver] = useState(false);

	const childrenCacheRef = useRef<Map<number | null, FolderInfo[]>>(new Map());
	const inflightLoadsRef = useRef<Map<number | null, Promise<void>>>(new Map());
	const expandingPathRef = useRef<string>("");

	useEffect(() => {
		if (folderTreeSnapshot?.userId === userId) return;
		folderTreeSnapshot = null;
		childrenCacheRef.current = new Map();
		inflightLoadsRef.current = new Map();
		expandingPathRef.current = "";
		setNodeMap(new Map());
		setRootIds([]);
		setExpandedIds(new Set());
		setLoadingIds(new Set());
		setLoadedIds(new Set());
		setRootLoaded(false);
	}, [userId]);

	useEffect(() => {
		folderTreeSnapshot = {
			expandedIds: Array.from(expandedIds),
			loadedIds: Array.from(loadedIds),
			nodeEntries: cloneNodeEntries(nodeMap),
			rootIds,
			userId,
		};
	}, [expandedIds, loadedIds, nodeMap, rootIds, userId]);

	const syncFolderChildren = useCallback(
		(parentId: number | null, folders: FolderInfo[]) => {
			childrenCacheRef.current.set(parentId, folders);
			setNodeMap((prev) => upsertChildren(prev, parentId, folders).nodeMap);
			if (parentId === null) {
				setRootIds(folders.map((folder) => folder.id));
				setRootLoaded(true);
			} else {
				setLoadedIds((prev) => new Set(prev).add(parentId));
			}
		},
		[],
	);

	const ensureChildrenLoaded = useCallback(
		async (parentId: number | null) => {
			if (parentId === null) {
				if (rootLoaded) return;
			} else if (loadedIds.has(parentId)) {
				return;
			}

			const inflight = inflightLoadsRef.current.get(parentId);
			if (inflight) {
				await inflight;
				return;
			}

			const cached = childrenCacheRef.current.get(parentId);
			if (cached) {
				syncFolderChildren(parentId, cached);
				return;
			}

			const loadPromise = (async () => {
				if (parentId !== null) {
					setLoadingIds((prev) => new Set(prev).add(parentId));
				}
				try {
					const contents =
						parentId === null
							? await fileService.listRoot({ file_limit: 0, folder_limit: 1000 })
							: await fileService.listFolder(parentId, {
									file_limit: 0,
									folder_limit: 1000,
							  });
					syncFolderChildren(parentId, contents.folders);
				} finally {
					if (parentId !== null) {
						setLoadingIds((prev) => {
							const next = new Set(prev);
							next.delete(parentId);
							return next;
						});
					}
					inflightLoadsRef.current.delete(parentId);
				}
			})();

			inflightLoadsRef.current.set(parentId, loadPromise);
			await loadPromise;
		},
		[loadedIds, rootLoaded, syncFolderChildren],
	);

	useEffect(() => {
		if (rootLoaded) return;
		let cancelled = false;
		void ensureChildrenLoaded(null).catch(() => {
			if (!cancelled) {
				setRootLoaded(false);
			}
		});
		return () => {
			cancelled = true;
		};
	}, [ensureChildrenLoaded, rootLoaded]);

	useEffect(() => {
		if (storeLoading) return;
		if (rootLoaded && storeCurrentFolderId === null && isRootRoute) {
			syncFolderChildren(null, storeFolders);
		}
	}, [
		isRootRoute,
		rootLoaded,
		storeCurrentFolderId,
		storeFolders,
		storeLoading,
		syncFolderChildren,
	]);

	useEffect(() => {
		if (storeLoading) return;
		if (!rootLoaded || storeCurrentFolderId === null) return;
		syncFolderChildren(storeCurrentFolderId, storeFolders);
	}, [rootLoaded, storeCurrentFolderId, storeFolders, storeLoading, syncFolderChildren]);

	useEffect(() => {
		if (!rootLoaded || currentFolderId === null) return;

		const pathIds = breadcrumb
			.map((item) => item.id)
			.filter((id): id is number => id !== null);
		if (pathIds.length === 0) return;

		const pathKey = pathIds.join("/");
		if (expandingPathRef.current === pathKey) return;

		let cancelled = false;

		async function expandPath() {
			for (let i = 0; i < pathIds.length - 1; i++) {
				const folderId = pathIds[i];
				if (cancelled) return;
				await ensureChildrenLoaded(folderId);
				if (cancelled) return;
				setExpandedIds((prev) => new Set(prev).add(folderId));
			}
			expandingPathRef.current = pathKey;
		}

		void expandPath();
		return () => {
			cancelled = true;
		};
	}, [breadcrumb, currentFolderId, ensureChildrenLoaded, rootLoaded]);

	useEffect(() => {
		expandingPathRef.current = "";
	}, [currentFolderId]);

	const handleToggle = useCallback(
		async (folderId: number) => {
			if (expandedIds.has(folderId)) {
				setExpandedIds((prev) => {
					const next = new Set(prev);
					next.delete(folderId);
					return next;
				});
				return;
			}

			await ensureChildrenLoaded(folderId);
			setExpandedIds((prev) => new Set(prev).add(folderId));
		},
		[ensureChildrenLoaded, expandedIds],
	);

	const handleNavigate = useCallback(
		async (id: number, name: string) => {
			await ensureChildrenLoaded(id);
			setExpandedIds((prev) => new Set(prev).add(id));
			navigate(`/folder/${id}?name=${encodeURIComponent(name)}`);
		},
		[ensureChildrenLoaded, navigate],
	);

	const handleDrop = useCallback(
		(fileIds: number[], folderIds: number[], targetFolderId: number) => {
			moveToFolder(fileIds, folderIds, targetFolderId)
				.then((result) => {
					const batchToast = formatBatchToast(t, "move", result);
					if (batchToast.variant === "error") {
						toast.error(batchToast.title, {
							description: batchToast.description,
						});
					} else {
						toast.success(batchToast.title, {
							description: batchToast.description,
						});
					}
				})
				.catch(handleApiError);
		},
		[moveToFolder, t],
	);

	const handleRootDragOver = (e: React.DragEvent) => {
		if (!e.dataTransfer.types.includes(DRAG_MIME)) return;
		e.preventDefault();
		e.dataTransfer.dropEffect = "move";
		setRootDragOver(true);
	};

	const handleRootDrop = (e: React.DragEvent) => {
		setRootDragOver(false);
		e.preventDefault();
		const raw = e.dataTransfer.getData(DRAG_MIME);
		if (!raw) return;
		const data = JSON.parse(raw) as { fileIds: number[]; folderIds: number[] };
		moveToFolder(data.fileIds, data.folderIds, null)
			.then((result) => {
				const batchToast = formatBatchToast(t, "move", result);
				if (batchToast.variant === "error") {
					toast.error(batchToast.title, {
						description: batchToast.description,
					});
				} else {
					toast.success(batchToast.title, {
						description: batchToast.description,
					});
				}
			})
			.catch(handleApiError);
	};

	function renderChildren(ids: number[], depth: number): React.ReactNode {
		return ids.map((id) => (
			<TreeNode
				key={id}
				currentFolderId={currentFolderId}
				depth={depth}
				expandedIds={expandedIds}
				loadedIds={loadedIds}
				loadingIds={loadingIds}
				nodeId={id}
				nodeMap={nodeMap}
				onDrop={handleDrop}
				onNavigate={handleNavigate}
				onToggle={handleToggle}
				renderChildren={renderChildren}
			/>
		));
	}

	const visibleRootIds = useMemo(() => rootIds.filter((id) => nodeMap.has(id)), [nodeMap, rootIds]);

	return (
		<div className="p-2 space-y-0.5">
			{!rootLoaded ? (
				<SkeletonTree count={4} />
			) : (
				<>
					<button
						type="button"
						className={cn(
							"w-full flex items-center gap-2 px-2 py-1.5 rounded-md text-sm hover:bg-accent transition-colors text-left",
							currentFolderId === null &&
								(location.pathname === "/" || location.pathname.startsWith("/folder")) &&
								"bg-accent font-medium",
							rootDragOver && "ring-2 ring-primary bg-accent/30",
						)}
						onClick={() => navigate("/")}
						onDragOver={handleRootDragOver}
						onDragLeave={() => setRootDragOver(false)}
						onDrop={handleRootDrop}
					>
						<Icon name="Folder" className="h-4 w-4 text-muted-foreground" />
						{t("root")}
					</button>
					{renderChildren(visibleRootIds, 1)}
				</>
			)}
		</div>
	);
}
