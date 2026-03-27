import { useCallback, useState } from "react";

interface UseConfirmDialogReturn<T> {
	confirmId: T | null;
	requestConfirm: (id: T) => void;
	dialogProps: {
		open: boolean;
		onOpenChange: (open: boolean) => void;
		onConfirm: () => void;
		confirmId: T | null;
	};
}

export function useConfirmDialog<T = number>(
	onConfirm: (id: T) => void | Promise<void>,
): UseConfirmDialogReturn<T> {
	const [confirmId, setConfirmId] = useState<T | null>(null);

	const requestConfirm = useCallback((id: T) => {
		setConfirmId(id);
	}, []);

	const dialogProps = {
		open: confirmId !== null,
		onOpenChange: (open: boolean) => {
			if (!open) setConfirmId(null);
		},
		onConfirm: () => {
			const id = confirmId;
			setConfirmId(null);
			if (id !== null) void onConfirm(id);
		},
		confirmId,
	};

	return { confirmId, requestConfirm, dialogProps };
}
