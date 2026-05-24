import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
	Dialog,
	DialogContent,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { handleApiError } from "@/hooks/useApiError";
import {
	loadAdminPolicyGroupLookup,
	readAdminPolicyGroupLookup,
} from "@/lib/adminPolicyGroupLookup";
import type {
	StoragePolicyGroup,
	UpdateUserRequest,
	UserInfo,
} from "@/types/api";
import { UserDetailDialogBody } from "./user-detail-dialog/UserDetailDialogBody";
import { userDetailDraftKey } from "./user-detail-dialog/userDetailDialogState";

interface UserDetailDialogProps {
	user: UserInfo | null;
	open: boolean;
	onOpenChange: (open: boolean) => void;
	onUpdate: (id: number, data: UpdateUserRequest) => Promise<void>;
}

export function UserDetailDialog({
	user: inputUser,
	open,
	onOpenChange,
	onUpdate,
}: UserDetailDialogProps) {
	const { t } = useTranslation(["admin", "core"]);
	const retainedUserRef = useRef<UserInfo | null>(inputUser);
	const [retentionVersion, setRetentionVersion] = useState(0);

	if (inputUser !== null) {
		retainedUserRef.current = inputUser;
	}

	const user = inputUser ?? (open ? null : retainedUserRef.current);
	const handleOpenChangeComplete = useCallback((nextOpen: boolean) => {
		if (!nextOpen) {
			retainedUserRef.current = null;
			setRetentionVersion((version) => version + 1);
		}
	}, []);

	if (!user) return null;

	return (
		<Dialog
			key={retentionVersion}
			open={open}
			onOpenChange={onOpenChange}
			onOpenChangeComplete={handleOpenChangeComplete}
		>
			<DialogContent
				keepMounted
				className="flex max-h-[min(860px,calc(100vh-2rem))] flex-col gap-0 overflow-hidden p-0 sm:max-w-[min(1100px,calc(100vw-2rem))]"
			>
				<DialogHeader className="shrink-0 px-6 pt-5 pb-0 text-center max-lg:px-4 max-lg:pt-4">
					<DialogTitle className="text-lg">{t("user_details")}</DialogTitle>
				</DialogHeader>
				<UserDetailDialogLoadedBody
					onClose={() => onOpenChange(false)}
					onUpdate={onUpdate}
					user={user}
				/>
			</DialogContent>
		</Dialog>
	);
}

function UserDetailDialogLoadedBody({
	onClose,
	onUpdate,
	user,
}: {
	onClose: () => void;
	onUpdate: (id: number, data: UpdateUserRequest) => Promise<void>;
	user: UserInfo;
}) {
	const initialPolicyGroups = readAdminPolicyGroupLookup();
	const [policyGroups, setPolicyGroups] = useState<StoragePolicyGroup[]>(
		initialPolicyGroups ?? [],
	);
	const [policyGroupsLoading, setPolicyGroupsLoading] = useState(
		initialPolicyGroups == null,
	);

	const loadPolicyGroups = useCallback(
		async (options?: { force?: boolean }) => {
			try {
				const cachedPolicyGroups = readAdminPolicyGroupLookup();
				if (!options?.force && cachedPolicyGroups != null) {
					setPolicyGroups(cachedPolicyGroups);
					setPolicyGroupsLoading(false);
				} else {
					setPolicyGroupsLoading(true);
				}
				setPolicyGroups(await loadAdminPolicyGroupLookup(options));
			} catch (e) {
				handleApiError(e);
			} finally {
				setPolicyGroupsLoading(false);
			}
		},
		[],
	);

	useEffect(() => {
		void loadPolicyGroups();
	}, [loadPolicyGroups]);

	return (
		<UserDetailDialogBody
			key={userDetailDraftKey(user)}
			onClose={onClose}
			onRefreshPolicyGroups={() => loadPolicyGroups({ force: true })}
			onUpdate={onUpdate}
			policyGroups={policyGroups}
			policyGroupsLoading={policyGroupsLoading}
			user={user}
		/>
	);
}
