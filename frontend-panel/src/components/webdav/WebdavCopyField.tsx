import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";

interface WebdavCopyFieldProps {
	copyLabel?: string;
	onCopy: () => void;
	value: string;
}

export function WebdavCopyField({
	copyLabel,
	onCopy,
	value,
}: WebdavCopyFieldProps) {
	return (
		<div className="flex flex-col gap-2 sm:flex-row">
			<Input readOnly value={value} className="font-mono" />
			<Button
				type="button"
				variant="outline"
				size={copyLabel ? "default" : "icon-sm"}
				className="sm:shrink-0"
				onClick={onCopy}
			>
				<Icon name="Copy" className="size-3.5" />
				{copyLabel ? copyLabel : null}
			</Button>
		</div>
	);
}
