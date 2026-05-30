import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { getUserDisplayName } from "@/lib/user";
import type { TeamMemberInfo } from "@/types/api";

interface TeamManageConfirmDialogsProps {
	archiveConfirmLabel: string;
	archiveDescription: string;
	archiveDialogProps: {
		onOpenChange: (open: boolean) => void;
		open: boolean;
		onConfirm: () => void;
	};
	currentUserId: number | null;
	leaveLabel: string;
	removeDescription: string;
	removeDialogProps: {
		onOpenChange: (open: boolean) => void;
		open: boolean;
		onConfirm: () => void;
	};
	removeLabel: string;
	removeMember: TeamMemberInfo | null;
}

export function TeamManageConfirmDialogs({
	archiveConfirmLabel,
	archiveDescription,
	archiveDialogProps,
	currentUserId,
	leaveLabel,
	removeDescription,
	removeDialogProps,
	removeLabel,
	removeMember,
}: TeamManageConfirmDialogsProps) {
	return (
		<>
			<ConfirmDialog
				{...removeDialogProps}
				title={
					removeMember?.user_id === currentUserId ? leaveLabel : removeLabel
				}
				description={
					removeMember
						? `${removeDescription} ${getUserDisplayName(removeMember.user)}`
						: removeDescription
				}
				confirmLabel={
					removeMember?.user_id === currentUserId ? leaveLabel : removeLabel
				}
				variant="destructive"
			/>

			<ConfirmDialog
				{...archiveDialogProps}
				title={archiveConfirmLabel}
				description={archiveDescription}
				confirmLabel={archiveConfirmLabel}
				variant="destructive"
			/>
		</>
	);
}
