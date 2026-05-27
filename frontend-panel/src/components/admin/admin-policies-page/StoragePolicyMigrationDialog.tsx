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
import { Label } from "@/components/ui/label";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import type { StoragePolicy } from "@/types/api";

interface StoragePolicyMigrationDialogProps {
	open: boolean;
	policies: StoragePolicy[];
	sourcePolicyId: string;
	submitting: boolean;
	targetPolicyId: string;
	onOpenChange: (open: boolean) => void;
	onSourcePolicyChange: (policyId: string) => void;
	onSubmit: () => void;
	onTargetPolicyChange: (policyId: string) => void;
}

function policyOptionLabel(policy: StoragePolicy) {
	return `#${policy.id} · ${policy.name}`;
}

function selectedPolicyLabel(policies: StoragePolicy[], policyId: string) {
	const policy = policies.find((item) => String(item.id) === policyId);
	return policy ? policyOptionLabel(policy) : undefined;
}

export function StoragePolicyMigrationDialog({
	open,
	policies,
	sourcePolicyId,
	submitting,
	targetPolicyId,
	onOpenChange,
	onSourcePolicyChange,
	onSubmit,
	onTargetPolicyChange,
}: StoragePolicyMigrationDialogProps) {
	const { t } = useTranslation("admin");
	const sourceId = Number(sourcePolicyId);
	const targetId = Number(targetPolicyId);
	const sourceLabel = selectedPolicyLabel(policies, sourcePolicyId);
	const targetLabel = selectedPolicyLabel(policies, targetPolicyId);
	const canSubmit =
		Number.isSafeInteger(sourceId) &&
		Number.isSafeInteger(targetId) &&
		sourceId > 0 &&
		targetId > 0 &&
		sourceId !== targetId &&
		!submitting;

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="sm:max-w-[34rem]">
				<DialogHeader>
					<DialogTitle>{t("policy_migration_title")}</DialogTitle>
					<DialogDescription>{t("policy_migration_desc")}</DialogDescription>
				</DialogHeader>

				<div className="space-y-4">
					<div className="grid gap-4 sm:grid-cols-2">
						<div className="space-y-2">
							<Label htmlFor="storage-migration-source">
								{t("policy_migration_source")}
							</Label>
							<Select
								value={sourcePolicyId}
								onValueChange={(value) => {
									if (value) onSourcePolicyChange(value);
								}}
								disabled={submitting}
							>
								<SelectTrigger id="storage-migration-source">
									<SelectValue
										placeholder={t("policy_migration_select_source")}
									>
										{sourceLabel}
									</SelectValue>
								</SelectTrigger>
								<SelectContent>
									{policies.map((policy) => (
										<SelectItem key={policy.id} value={String(policy.id)}>
											<span className="truncate">
												{policyOptionLabel(policy)}
											</span>
										</SelectItem>
									))}
								</SelectContent>
							</Select>
						</div>

						<div className="space-y-2">
							<Label htmlFor="storage-migration-target">
								{t("policy_migration_target")}
							</Label>
							<Select
								value={targetPolicyId}
								onValueChange={(value) => {
									if (value) onTargetPolicyChange(value);
								}}
								disabled={submitting}
							>
								<SelectTrigger id="storage-migration-target">
									<SelectValue
										placeholder={t("policy_migration_select_target")}
									>
										{targetLabel}
									</SelectValue>
								</SelectTrigger>
								<SelectContent>
									{policies.map((policy) => (
										<SelectItem
											key={policy.id}
											value={String(policy.id)}
											disabled={String(policy.id) === sourcePolicyId}
										>
											<span className="truncate">
												{policyOptionLabel(policy)}
											</span>
										</SelectItem>
									))}
								</SelectContent>
							</Select>
						</div>
					</div>

					{sourcePolicyId &&
					targetPolicyId &&
					sourcePolicyId === targetPolicyId ? (
						<div className="rounded-lg border border-destructive/25 bg-destructive/5 px-3 py-2 text-sm text-destructive">
							{t("policy_migration_same_policy_error")}
						</div>
					) : null}
				</div>

				<DialogFooter>
					<Button
						type="button"
						variant="outline"
						onClick={() => onOpenChange(false)}
						disabled={submitting}
					>
						{t("core:cancel")}
					</Button>
					<Button type="button" onClick={onSubmit} disabled={!canSubmit}>
						<Icon
							name={submitting ? "Spinner" : "ArrowsClockwise"}
							className={`mr-1 size-4 ${submitting ? "animate-spin" : ""}`}
						/>
						{submitting
							? t("policy_migration_creating")
							: t("policy_migration_submit")}
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
