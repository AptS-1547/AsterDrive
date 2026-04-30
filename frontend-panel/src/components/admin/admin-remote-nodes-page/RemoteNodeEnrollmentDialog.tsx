import { useTranslation } from "react-i18next";
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
import { formatDateTime, formatDateTimeWithOffset } from "@/lib/format";
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
			<DialogContent className="flex max-h-[min(90vh,calc(100vh-2rem))] flex-col gap-0 overflow-hidden p-0 sm:max-w-[calc(100%-2rem)] lg:max-w-4xl">
				<DialogHeader className="shrink-0 border-b px-6 pt-5 pb-4 pr-14">
					<DialogTitle>{t("remote_node_enrollment_dialog_title")}</DialogTitle>
					<DialogDescription>
						{t("remote_node_enrollment_dialog_desc")}
					</DialogDescription>
				</DialogHeader>

				<div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
					<div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_18rem]">
						<div className="min-w-0 space-y-6">
							<section className="space-y-3">
								<div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
									<div className="min-w-0">
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
								<pre className="max-h-44 overflow-x-auto whitespace-pre rounded-lg border border-border/70 bg-muted/20 p-3 font-mono text-xs leading-6 text-foreground">
									{command.command}
								</pre>
								<p className="text-xs leading-5 text-muted-foreground">
									{t("remote_node_enrollment_command_hint")}
								</p>
							</section>

							<section className="border-t pt-5">
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

						<aside className="space-y-5 lg:border-l lg:pl-6">
							<section>
								<h3 className="text-sm font-semibold text-foreground">
									{t("remote_node_enrollment_details_title")}
								</h3>
								<dl className="mt-3 space-y-3">
									<div>
										<dt className="text-[11px] font-medium uppercase tracking-[0.16em] text-muted-foreground">
											{t("core:name")}
										</dt>
										<dd className="mt-1 break-all text-sm font-medium text-foreground">
											{command.remote_node_name}
										</dd>
									</div>
									<div>
										<dt className="text-[11px] font-medium uppercase tracking-[0.16em] text-muted-foreground">
											{t("remote_node_enrollment_master_url")}
										</dt>
										<dd className="mt-1 break-all text-sm font-medium text-foreground">
											{command.master_url}
										</dd>
									</div>
									<div>
										<dt className="text-[11px] font-medium uppercase tracking-[0.16em] text-muted-foreground">
											{t("remote_node_enrollment_expires_at")}
										</dt>
										<dd
											className="mt-1 text-sm font-medium text-foreground"
											title={formatDateTimeWithOffset(command.expires_at)}
										>
											{formatDateTime(command.expires_at)}
										</dd>
									</div>
								</dl>
							</section>

							<section className="border-t pt-5">
								<h3 className="text-sm font-semibold text-foreground">
									{t("remote_node_enrollment_flow_title")}
								</h3>
								<p className="mt-1 text-xs leading-5 text-muted-foreground">
									{t("remote_node_enrollment_flow_desc")}
								</p>
								<ol className="mt-4 space-y-4">
									{[
										{
											title: t("remote_node_enrollment_step_issue_title"),
											description: t("remote_node_enrollment_step_issue_desc"),
										},
										{
											title: t("remote_node_enrollment_step_run_title"),
											description: t("remote_node_enrollment_step_run_desc"),
										},
										{
											title: t("remote_node_enrollment_step_restart_title"),
											description: t(
												"remote_node_enrollment_step_restart_desc",
											),
										},
									].map((step, index) => (
										<li
											key={step.title}
											className="grid grid-cols-[1.5rem_minmax(0,1fr)] gap-3"
										>
											<span className="flex h-6 w-6 items-center justify-center rounded-full border border-border/80 bg-background text-[11px] font-medium text-muted-foreground">
												{index + 1}
											</span>
											<div className="min-w-0">
												<p className="text-sm font-medium text-foreground">
													{step.title}
												</p>
												<p className="mt-1 text-xs leading-5 text-muted-foreground">
													{step.description}
												</p>
											</div>
										</li>
									))}
								</ol>
							</section>
						</aside>
					</div>
				</div>
				<DialogFooter className="mx-0 mb-0 w-full shrink-0 flex-row items-center gap-2 rounded-b-xl px-6 py-3">
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
