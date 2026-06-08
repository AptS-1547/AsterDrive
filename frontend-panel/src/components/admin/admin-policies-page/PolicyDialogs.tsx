import { useTranslation } from "react-i18next";
import { StoragePolicyDialog } from "@/components/admin/StoragePolicyDialog";
import type { PolicyFormData } from "@/components/admin/storagePolicyDialogShared";
import type { ConfirmDialogProps } from "@/components/common/ConfirmDialog";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import type {
	DriverType,
	RemoteNodeInfo,
	StoragePolicyCapacityInfo,
} from "@/types/api";

interface PolicyDialogsProps {
	createStep: number;
	createStepTouched: boolean;
	deleteDialogProps: Pick<
		ConfirmDialogProps,
		"onConfirm" | "onOpenChange" | "open"
	>;
	deletePolicyName: string;
	forceDeleteDialogProps: Pick<
		ConfirmDialogProps,
		"onConfirm" | "onOpenChange" | "open"
	>;
	forceDeletePolicyName: string;
	dialogOpen: boolean;
	editMode: boolean;
	endpointValidationMessage: string | null;
	form: PolicyFormData;
	policyCapacity: StoragePolicyCapacityInfo | null;
	policyCapacityLoading: boolean;
	remoteNodes: RemoteNodeInfo[];
	saveAnywayConfirmOpen: boolean;
	submitting: boolean;
	onCancelSaveAnyway: () => void;
	onConfirmSaveAnyway: () => void;
	onCreateBack: () => void;
	onCreateNext: () => void;
	onCreateStepChange: (step: number) => void;
	onDialogOpenChange: (open: boolean) => void;
	onDriverTypeChange: (driverType: DriverType) => void;
	onFieldChange: <K extends keyof PolicyFormData>(
		key: K,
		value: PolicyFormData[K],
	) => void;
	onRunConnectionTest: () => Promise<boolean>;
	onSubmit: () => void;
	onSyncNormalizedS3Form: () => void;
}

export function PolicyDialogs({
	createStep,
	createStepTouched,
	deleteDialogProps,
	deletePolicyName,
	forceDeleteDialogProps,
	forceDeletePolicyName,
	dialogOpen,
	editMode,
	endpointValidationMessage,
	form,
	policyCapacity,
	policyCapacityLoading,
	remoteNodes,
	saveAnywayConfirmOpen,
	submitting,
	onCancelSaveAnyway,
	onConfirmSaveAnyway,
	onCreateBack,
	onCreateNext,
	onCreateStepChange,
	onDialogOpenChange,
	onDriverTypeChange,
	onFieldChange,
	onRunConnectionTest,
	onSubmit,
	onSyncNormalizedS3Form,
}: PolicyDialogsProps) {
	const { t } = useTranslation("admin");

	return (
		<>
			<ConfirmDialog
				{...deleteDialogProps}
				title={`${t("delete_policy")} "${deletePolicyName}"?`}
				description={t("delete_policy_desc")}
				confirmLabel={t("core:delete")}
				variant="destructive"
			/>
			<ConfirmDialog
				{...forceDeleteDialogProps}
				title={`${t("force_delete_policy")} "${forceDeletePolicyName}"?`}
				description={t("force_delete_policy_desc")}
				confirmLabel={t("force_delete_policy_confirm")}
				variant="destructive"
			/>
			<StoragePolicyDialog
				open={dialogOpen}
				mode={editMode ? "edit" : "create"}
				form={form}
				policyCapacity={policyCapacity}
				policyCapacityLoading={policyCapacityLoading}
				remoteNodes={remoteNodes}
				submitting={submitting}
				createStep={createStep}
				createStepTouched={createStepTouched}
				endpointValidationMessage={endpointValidationMessage}
				saveAnywayConfirmOpen={saveAnywayConfirmOpen}
				onOpenChange={onDialogOpenChange}
				onCancelSaveAnyway={onCancelSaveAnyway}
				onConfirmSaveAnyway={onConfirmSaveAnyway}
				onSubmit={onSubmit}
				onRunConnectionTest={onRunConnectionTest}
				onFieldChange={onFieldChange}
				onDriverTypeChange={onDriverTypeChange}
				onCreateBack={onCreateBack}
				onCreateStepChange={onCreateStepChange}
				onCreateNext={onCreateNext}
				onSyncNormalizedS3Form={onSyncNormalizedS3Form}
			/>
		</>
	);
}
