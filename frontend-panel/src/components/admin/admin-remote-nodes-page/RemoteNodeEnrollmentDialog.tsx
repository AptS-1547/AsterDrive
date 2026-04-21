import { useTranslation } from "react-i18next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Icon } from "@/components/ui/icon";
import { ADMIN_CONTROL_HEIGHT_CLASS } from "@/lib/constants";
import { formatDateTime } from "@/lib/format";
import type { RemoteEnrollmentCommandInfo } from "@/types/api";
import { TestConnectionButton } from "./shared";

interface RemoteNodeEnrollmentDialogProps {
	canTestConnection: boolean;
	command: RemoteEnrollmentCommandInfo | null;
	onCopy: (value: string) => Promise<void>;
	onOpenChange: (open: boolean) => void;
	onVerifyConnection: (remoteNodeId: number) => Promise<boolean>;
	open: boolean;
}

export function RemoteNodeEnrollmentDialog({
	canTestConnection,
	command,
	onCopy,
	onOpenChange,
	onVerifyConnection,
	open,
}: RemoteNodeEnrollmentDialogProps) {
	const { t } = useTranslation("admin");

	if (!command) {
		return null;
	}

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="max-h-[min(90vh,calc(100vh-2rem))] overflow-y-auto sm:max-w-3xl">
				<DialogHeader>
					<DialogTitle>{t("remote_node_enrollment_dialog_title")}</DialogTitle>
					<DialogDescription>
						{t("remote_node_enrollment_dialog_desc")}
					</DialogDescription>
				</DialogHeader>

				<div className="space-y-5">
					<section className="rounded-2xl border border-blue-500/20 bg-blue-500/5 p-5">
						<div className="flex items-start gap-3">
							<div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-2xl bg-background/80 ring-1 ring-blue-500/10">
								<Icon
									name="ClipboardText"
									className="h-5 w-5 text-blue-600 dark:text-blue-300"
								/>
							</div>
							<div className="min-w-0">
								<h3 className="text-sm font-semibold text-foreground">
									{t("remote_node_enrollment_saved_title")}
								</h3>
								<p className="mt-1 text-sm leading-6 text-muted-foreground">
									{t("remote_node_enrollment_saved_desc")}
								</p>
							</div>
						</div>
					</section>

					<div className="grid gap-3 md:grid-cols-3">
						<div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
							<p className="text-[11px] font-medium uppercase tracking-[0.16em] text-muted-foreground">
								{t("core:name")}
							</p>
							<p className="mt-2 break-all text-sm font-medium text-foreground">
								{command.remote_node_name}
							</p>
						</div>
						<div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
							<p className="text-[11px] font-medium uppercase tracking-[0.16em] text-muted-foreground">
								{t("remote_node_enrollment_master_url")}
							</p>
							<p className="mt-2 break-all text-sm font-medium text-foreground">
								{command.master_url}
							</p>
						</div>
						<div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
							<p className="text-[11px] font-medium uppercase tracking-[0.16em] text-muted-foreground">
								{t("remote_node_enrollment_expires_at")}
							</p>
							<p className="mt-2 text-sm font-medium text-foreground">
								{formatDateTime(command.expires_at)}
							</p>
						</div>
					</div>

					<section className="rounded-2xl border border-border/70 bg-background/70 p-5">
						<div className="mb-4">
							<h3 className="text-base font-semibold text-foreground">
								{t("remote_node_enrollment_flow_title")}
							</h3>
							<p className="mt-1 text-sm text-muted-foreground">
								{t("remote_node_enrollment_flow_desc")}
							</p>
						</div>
						<div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_96px_minmax(0,1fr)_96px_minmax(0,1fr)] md:items-center">
							<div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
								<p className="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">
									1
								</p>
								<p className="mt-2 font-semibold text-foreground">
									{t("remote_node_enrollment_step_issue_title")}
								</p>
								<p className="mt-2 text-xs leading-5 text-muted-foreground">
									{t("remote_node_enrollment_step_issue_desc")}
								</p>
							</div>
							<div className="flex justify-center">
								<Badge
									variant="outline"
									className="rounded-full border-foreground/15 px-3 py-1 text-[11px] font-medium"
								>
									{t("remote_node_enrollment_step_arrow_issue")}
								</Badge>
							</div>
							<div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
								<p className="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">
									2
								</p>
								<p className="mt-2 font-semibold text-foreground">
									{t("remote_node_enrollment_step_run_title")}
								</p>
								<p className="mt-2 text-xs leading-5 text-muted-foreground">
									{t("remote_node_enrollment_step_run_desc")}
								</p>
							</div>
							<div className="flex justify-center">
								<Badge
									variant="outline"
									className="rounded-full border-foreground/15 px-3 py-1 text-[11px] font-medium"
								>
									{t("remote_node_enrollment_step_arrow_run")}
								</Badge>
							</div>
							<div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
								<p className="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">
									3
								</p>
								<p className="mt-2 font-semibold text-foreground">
									{t("remote_node_enrollment_step_restart_title")}
								</p>
								<p className="mt-2 text-xs leading-5 text-muted-foreground">
									{t("remote_node_enrollment_step_restart_desc")}
								</p>
							</div>
						</div>
					</section>

					<section className="rounded-2xl border border-border/70 bg-background/70 p-5">
						<div className="flex items-start justify-between gap-3">
							<div>
								<h3 className="text-base font-semibold text-foreground">
									{t("remote_node_enrollment_command_title")}
								</h3>
								<p className="mt-1 text-sm text-muted-foreground">
									{t("remote_node_enrollment_command_desc")}
								</p>
							</div>
							<Button
								type="button"
								variant="outline"
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								onClick={() => void onCopy(command.command)}
							>
								<Icon name="Copy" className="mr-1 h-4 w-4" />
								{t("remote_node_enrollment_copy_command")}
							</Button>
						</div>
						<pre className="mt-4 overflow-x-auto whitespace-pre-wrap break-all rounded-2xl bg-muted/20 p-4 font-mono text-xs leading-6 text-foreground">
							{command.command}
						</pre>
						<p className="mt-3 text-xs leading-5 text-muted-foreground">
							{t("remote_node_enrollment_command_hint")}
						</p>
					</section>

					<section className="rounded-2xl border border-border/70 bg-background/70 p-5">
						<div className="flex flex-wrap items-start justify-between gap-3">
							<div className="min-w-0">
								<h3 className="text-base font-semibold text-foreground">
									{t("remote_node_enrollment_verify_title")}
								</h3>
								<p className="mt-1 text-sm text-muted-foreground">
									{t("remote_node_enrollment_verify_desc")}
								</p>
							</div>
							<TestConnectionButton
								onTest={() => onVerifyConnection(command.remote_node_id)}
								disabled={!canTestConnection}
							/>
						</div>
						<p className="mt-3 text-xs leading-5 text-muted-foreground">
							{canTestConnection
								? t("remote_node_enrollment_verify_hint")
								: t("remote_node_enrollment_verify_disabled_hint")}
						</p>
					</section>
				</div>
				<DialogFooter className="px-0 pb-0">
					<Button
						type="button"
						variant="outline"
						className={ADMIN_CONTROL_HEIGHT_CLASS}
						onClick={() => onOpenChange(false)}
					>
						{t("core:close")}
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
