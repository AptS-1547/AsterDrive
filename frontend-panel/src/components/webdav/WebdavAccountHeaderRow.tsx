import { TableHead, TableHeader, TableRow } from "@/components/ui/table";

interface WebdavAccountHeaderRowProps {
	actionLabel: string;
	createdAtLabel: string;
	ownerLabel?: string;
	statusLabel: string;
	usernameLabel: string;
	scopeLabel: string;
}

export function WebdavAccountHeaderRow({
	actionLabel,
	createdAtLabel,
	ownerLabel,
	statusLabel,
	usernameLabel,
	scopeLabel,
}: WebdavAccountHeaderRowProps) {
	return (
		<TableHeader>
			<TableRow>
				<TableHead>{usernameLabel}</TableHead>
				{ownerLabel ? <TableHead>{ownerLabel}</TableHead> : null}
				<TableHead>{scopeLabel}</TableHead>
				<TableHead>{statusLabel}</TableHead>
				<TableHead>{createdAtLabel}</TableHead>
				<TableHead className="w-[96px] text-right">{actionLabel}</TableHead>
			</TableRow>
		</TableHeader>
	);
}
