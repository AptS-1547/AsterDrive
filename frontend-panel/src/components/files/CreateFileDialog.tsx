import type { FormEvent } from "react";
import { useState } from "react";
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

interface CreateFileDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
}

export function CreateFileDialog({
	open,
	onOpenChange,
}: CreateFileDialogProps) {
	const { t } = useTranslation("files");
	const createFile = useFileStore((s) => s.createFile);
	const [name, setName] = useState("");

	const handleSubmit = async (e: FormEvent) => {
		e.preventDefault();
		if (!name.trim()) return;
		try {
			await createFile(name.trim());
			toast.success(t("create_file_success"));
			setName("");
			onOpenChange(false);
		} catch (err) {
			handleApiError(err);
		}
	};

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent>
				<DialogHeader>
					<DialogTitle>{t("create_file")}</DialogTitle>
				</DialogHeader>
				<form onSubmit={handleSubmit} className="space-y-4">
					<Input
						placeholder={t("file_name")}
						value={name}
						onChange={(e) => setName(e.target.value)}
						autoFocus
					/>
					<Button type="submit" className="w-full">
						{t("create_file")}
					</Button>
				</form>
			</DialogContent>
		</Dialog>
	);
}
