import type React from "react";
import { useCallback, useEffect, useState } from "react";
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

interface TreeNodeData {
	folder: FolderInfo;
	children: TreeNodeData[] | null; // null = not loaded
	expanded: boolean;
	loading: boolean;
}

interface FolderTreeCache {
	userId: number | null;
	nodes: TreeNodeData[];
	rootLoaded: boolean;
}

let folderTreeCache: FolderTreeCache | null = null;

function cloneTreeNodes(nodes: TreeNodeData[]): TreeNodeData[] {
	return nodes.map((node) => ({
		...node,
		children: node.children ? cloneTreeNodes(node.children) : null,
	}));
}

function createTreeNode(
	folder: FolderInfo,
	existing?: TreeNodeData,
): TreeNodeData {
	return existing
		? {
				...existing,
				folder,
			}
		: {
				folder,
				children: null,
				expanded: false,
				loading: false,
			};
}

function TreeNode({
	node,
	depth,
	onToggle,
	onNavigate,
	onDrop,
	currentFolderId,
}: {
	node: TreeNodeData;
	depth: number;
	onToggle: (id: number) => void;
	onNavigate: (id: number, name: string) => void;
	onDrop: (
		fileIds: number[],
		folderIds: number[],
		targetFolderId: number,
	) => void;
	currentFolderId: number | null;
}) {
	const isActive = currentFolderId === node.folder.id;
	const showToggle =
		node.loading || node.children === null || node.children.length > 0;
	const [dragOver, setDragOver] = useState(false);

	const handleDragOver = (e: React.DragEvent) => {
		if (!e.dataTransfer.types.includes(DRAG_MIME)) return;
		e.preventDefault();
		e.dataTransfer.dropEffect = "move";
		setDragOver(true);
	};

	const handleDragLeave = () => setDragOver(false);

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
						disabled={node.loading}
					>
						{node.loading ? (
							<div className="h-3 w-3 border-2 border-muted-foreground/30 border-t-muted-foreground rounded-full animate-spin" />
						) : node.expanded ? (
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
					{node.expanded ? (
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
			{node.expanded && node.children && node.children.length > 0 && (
				<div>
					{node.children.map((child) => (
						<TreeNode
							key={child.folder.id}
							node={child}
							depth={depth + 1}
							onToggle={onToggle}
							onNavigate={onNavigate}
							onDrop={onDrop}
							currentFolderId={currentFolderId}
						/>
					))}
				</div>
			)}
		</div>
	);
}

// Helper: recursively update a node in the tree
function updateNode(
	nodes: TreeNodeData[],
	targetId: number,
	updater: (node: TreeNodeData) => TreeNodeData,
): TreeNodeData[] {
	return nodes.map((n) => {
		if (n.folder.id === targetId) return updater(n);
		if (n.children) {
			return {
				...n,
				children: updateNode(n.children, targetId, updater),
			};
		}
		return n;
	});
}

export function FolderTree() {
	const { t } = useTranslation("files");
	const userId = useAuthStore((s) => s.user?.id ?? null);
	const location = useLocation();
	const currentFolderId = useFileStore((s) => s.currentFolderId);
	const moveToFolder = useFileStore((s) => s.moveToFolder);
	const storeFolders = useFileStore((s) => s.folders);
	const storeCurrentFolderId = useFileStore((s) => s.currentFolderId);
	const isRootRoute = location.pathname === "/";
	const cachedTree =
		folderTreeCache?.userId === userId ? folderTreeCache : null;
	const [nodes, setNodes] = useState<TreeNodeData[]>(() =>
		cachedTree ? cloneTreeNodes(cachedTree.nodes) : [],
	);
	const [rootLoaded, setRootLoaded] = useState(
		() => cachedTree?.rootLoaded ?? false,
	);

	useEffect(() => {
		if (folderTreeCache?.userId === userId) return;
		folderTreeCache = null;
		setNodes([]);
		setRootLoaded(false);
	}, [userId]);

	useEffect(() => {
		folderTreeCache = {
			userId,
			nodes: cloneTreeNodes(nodes),
			rootLoaded,
		};
	}, [nodes, rootLoaded, userId]);

	// Load root folders on mount (skip files — tree only needs folders)
	useEffect(() => {
		if (rootLoaded) return;

		let cancelled = false;

		const treeParams = { file_limit: 0, folder_limit: 1000 };

		async function loadRoot() {
			try {
				const contents = await fileService.listRoot(treeParams);
				if (cancelled) return;
				setNodes(contents.folders.map((f) => createTreeNode(f)));
				setRootLoaded(true);
			} catch {
				// Silently fail - file store will show errors
			}
		}
		void loadRoot();

		return () => {
			cancelled = true;
		};
	}, [rootLoaded]);

	// Refresh root when navigating to root and store folders change
	useEffect(() => {
		if (rootLoaded && storeCurrentFolderId === null && isRootRoute) {
			setNodes((prev) =>
				storeFolders.map((f) => {
					const existing = prev.find((n) => n.folder.id === f.id);
					return createTreeNode(f, existing);
				}),
			);
		}
	}, [isRootRoute, storeFolders, storeCurrentFolderId, rootLoaded]);

	useEffect(() => {
		if (!rootLoaded || storeCurrentFolderId === null) return;

		setNodes((prev) =>
			updateNode(prev, storeCurrentFolderId, (node) => {
				const existingChildren = node.children ?? [];
				const nextChildren = storeFolders.map((folder) =>
					createTreeNode(
						folder,
						existingChildren.find((child) => child.folder.id === folder.id),
					),
				);

				return {
					...node,
					loading: false,
					children: nextChildren,
					expanded:
						nextChildren.length > 0
							? node.expanded || currentFolderId === storeCurrentFolderId
							: false,
				};
			}),
		);
	}, [currentFolderId, rootLoaded, storeCurrentFolderId, storeFolders]);

	const handleToggle = useCallback(async (folderId: number) => {
		let shouldLoad = false;

		setNodes((prev) =>
			updateNode(prev, folderId, (n) => {
				if (n.loading) return n;

				if (n.expanded) {
					// Collapse
					return { ...n, expanded: false };
				}

				if (n.children !== null) {
					return {
						...n,
						expanded: n.children.length > 0,
					};
				}

				// Expand - need to load children
				shouldLoad = true;
				return { ...n, loading: true, expanded: true };
			}),
		);

		if (!shouldLoad) return;

		try {
			const contents = await fileService.listFolder(folderId, {
				file_limit: 0,
				folder_limit: 1000,
			});
			const childNodes = contents.folders.map((f) => createTreeNode(f));
			setNodes((prev) =>
				updateNode(prev, folderId, (n) => ({
					...n,
					loading: false,
					expanded: childNodes.length > 0,
					children: childNodes,
				})),
			);
		} catch {
			setNodes((prev) =>
				updateNode(prev, folderId, (n) => ({
					...n,
					loading: false,
					expanded: false,
				})),
			);
		}
	}, []);

	const navigate = useNavigate();

	const handleNavigate = useCallback(
		(id: number, name: string) => {
			navigate(`/folder/${id}?name=${encodeURIComponent(name)}`);
		},
		[navigate],
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

	// Root drop target state
	const [rootDragOver, setRootDragOver] = useState(false);
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

	return (
		<div className="p-2 space-y-0.5">
			{!rootLoaded ? (
				<SkeletonTree count={4} />
			) : (
				<>
					{/* Root */}
					<button
						type="button"
						className={cn(
							"w-full flex items-center gap-2 px-2 py-1.5 rounded-md text-sm hover:bg-accent transition-colors text-left",
							currentFolderId === null &&
								(location.pathname === "/" ||
									location.pathname.startsWith("/folder")) &&
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

					{/* Tree nodes */}
					{nodes.map((node) => (
						<TreeNode
							key={node.folder.id}
							node={node}
							depth={1}
							onToggle={handleToggle}
							onNavigate={handleNavigate}
							onDrop={handleDrop}
							currentFolderId={currentFolderId}
						/>
					))}
				</>
			)}
		</div>
	);
}
