import { useTranslation } from "react-i18next";
import { AdminTableList } from "@/components/common/AdminTableList";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import {
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import {
	ADMIN_ICON_BUTTON_CLASS,
	ADMIN_TABLE_ACTIONS_WIDTH_CLASS,
} from "@/lib/constants";
import { cn } from "@/lib/utils";
import type { RemoteNodeInfo } from "@/types/api";
import {
	formatLastChecked,
	getRemoteNodeStatusLabel,
	getRemoteNodeStatusTone,
	INTERACTIVE_TABLE_ROW_CLASS,
	REMOTE_NODE_BADGE_CELL_CONTENT_CLASS,
	REMOTE_NODE_TEXT_CELL_CONTENT_CLASS,
} from "./shared";

interface RemoteNodesTableProps {
	generatingEnrollmentId: number | null;
	items: RemoteNodeInfo[];
	loading: boolean;
	onEdit: (node: RemoteNodeInfo) => void;
	onGenerateEnrollmentCommand: (node: RemoteNodeInfo) => void;
	onRequestDelete: (id: number) => void;
}

export function RemoteNodesTable({
	generatingEnrollmentId,
	items,
	loading,
	onEdit,
	onGenerateEnrollmentCommand,
	onRequestDelete,
}: RemoteNodesTableProps) {
	const { t } = useTranslation("admin");

	return (
		<AdminTableList
			loading={loading}
			items={items}
			columns={5}
			rows={6}
			emptyTitle={t("no_remote_nodes")}
			emptyDescription={t("no_remote_nodes_desc")}
			headerRow={
				<TableHeader>
					<TableRow>
						<TableHead className="w-16">{t("id")}</TableHead>
						<TableHead>{t("core:name")}</TableHead>
						<TableHead>{t("base_url")}</TableHead>
						<TableHead>{t("remote_node_status")}</TableHead>
						<TableHead className={ADMIN_TABLE_ACTIONS_WIDTH_CLASS}>
							{t("core:actions")}
						</TableHead>
					</TableRow>
				</TableHeader>
			}
			renderRow={(node) => (
				<TableRow
					key={node.id}
					className={INTERACTIVE_TABLE_ROW_CLASS}
					onClick={() => onEdit(node)}
					onKeyDown={(event) => {
						if (event.key === "Enter" || event.key === " ") {
							event.preventDefault();
							onEdit(node);
						}
					}}
					tabIndex={0}
				>
					<TableCell>
						<div className={REMOTE_NODE_TEXT_CELL_CONTENT_CLASS}>
							<span className="font-mono text-xs text-muted-foreground">
								{node.id}
							</span>
						</div>
					</TableCell>
					<TableCell>
						<div className={REMOTE_NODE_TEXT_CELL_CONTENT_CLASS}>
							<div className="min-w-0">
								<div className="truncate font-medium text-foreground">
									{node.name}
								</div>
							</div>
						</div>
					</TableCell>
					<TableCell>
						<div className={REMOTE_NODE_TEXT_CELL_CONTENT_CLASS}>
							<span className="truncate text-xs font-mono text-muted-foreground">
								{node.base_url || t("remote_node_base_url_empty")}
							</span>
						</div>
					</TableCell>
					<TableCell>
						<div className={REMOTE_NODE_BADGE_CELL_CONTENT_CLASS}>
							<div className="space-y-2">
								<Badge
									variant="outline"
									className={getRemoteNodeStatusTone(node)}
								>
									{getRemoteNodeStatusLabel(t, node)}
								</Badge>
								<div className="text-xs text-muted-foreground">
									{formatLastChecked(t, node.last_checked_at)}
								</div>
							</div>
						</div>
					</TableCell>
					<TableCell
						onClick={(event) => event.stopPropagation()}
						onKeyDown={(event) => event.stopPropagation()}
					>
						<div className="flex justify-end gap-1">
							<Button
								variant="ghost"
								size="icon"
								className={ADMIN_ICON_BUTTON_CLASS}
								onClick={() => onGenerateEnrollmentCommand(node)}
								disabled={generatingEnrollmentId === node.id}
								aria-label={t("remote_node_generate_enrollment_command")}
								title={t("remote_node_generate_enrollment_command")}
							>
								<Icon
									name={
										generatingEnrollmentId === node.id
											? "Spinner"
											: "ClipboardText"
									}
									className={cn(
										"h-3.5 w-3.5",
										generatingEnrollmentId === node.id && "animate-spin",
									)}
								/>
							</Button>
							<TooltipProvider>
								<Tooltip>
									<TooltipTrigger>
										<div>
											<Button
												variant="ghost"
												size="icon"
												className={`${ADMIN_ICON_BUTTON_CLASS} text-destructive`}
												onClick={() => onRequestDelete(node.id)}
												aria-label={t("delete_remote_node")}
												title={t("delete_remote_node")}
											>
												<Icon name="Trash" className="h-3.5 w-3.5" />
											</Button>
										</div>
									</TooltipTrigger>
									{node.last_error ? (
										<TooltipContent>{node.last_error}</TooltipContent>
									) : null}
								</Tooltip>
							</TooltipProvider>
						</div>
					</TableCell>
				</TableRow>
			)}
		/>
	);
}
