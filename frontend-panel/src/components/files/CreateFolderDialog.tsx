import type { FormEvent } from "react";
import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { handleApiError } from "@/hooks/useApiError";
import { useFileStore } from "@/stores/fileStore";

interface CreateFolderDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
}

export function CreateFolderDialog({
	open,
	onOpenChange,
}: CreateFolderDialogProps) {
	const { t } = useTranslation("files");
	const createFolder = useFileStore((s) => s.createFolder);
	const [name, setName] = useState("");
	const [submitting, setSubmitting] = useState(false);
	const submittingRef = useRef(false);

	const handleSubmit = async (e: FormEvent) => {
		e.preventDefault();
		const trimmedName = name.trim();
		if (!trimmedName || submittingRef.current) return;
		submittingRef.current = true;
		setSubmitting(true);
		try {
			await createFolder(trimmedName);
			toast.success(t("create_folder_success"));
			setName("");
			onOpenChange(false);
		} catch (error) {
			handleApiError(error);
		} finally {
			submittingRef.current = false;
			setSubmitting(false);
		}
	};

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent keepMounted>
				<DialogHeader>
					<DialogTitle>{t("create_folder")}</DialogTitle>
				</DialogHeader>
				<form onSubmit={handleSubmit} className="space-y-4">
					<Input
						placeholder={t("folder_name")}
						value={name}
						onChange={(e) => setName(e.target.value)}
						autoFocus
					/>
					<Button type="submit" className="w-full" disabled={submitting}>
						{t("create_folder")}
					</Button>
				</form>
			</DialogContent>
		</Dialog>
	);
}
