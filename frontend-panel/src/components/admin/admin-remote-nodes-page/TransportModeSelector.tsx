import { cn } from "@/lib/utils";
import type { RemoteNodeTransportMode } from "../remoteNodeDialogShared";

export interface TransportModeOption {
	badge?: string;
	description: string;
	label: string;
	value: RemoteNodeTransportMode;
}

interface TransportModeSelectorProps {
	options: TransportModeOption[];
	value: RemoteNodeTransportMode;
	onChange: (value: RemoteNodeTransportMode) => void;
}

export function TransportModeSelector({
	options,
	value,
	onChange,
}: TransportModeSelectorProps) {
	return (
		<div className="grid gap-2 md:grid-cols-3">
			{options.map((option) => (
				<button
					key={option.value}
					type="button"
					aria-pressed={value === option.value}
					onClick={() => onChange(option.value)}
					className={cn(
						"min-h-24 rounded-xl border p-3 text-left transition",
						value === option.value
							? "border-primary bg-primary/5"
							: "border-border/70 bg-background hover:border-primary/40",
					)}
				>
					<span className="flex items-center gap-2 text-sm font-semibold text-foreground">
						<span>{option.label}</span>
						{option.badge ? (
							<span className="rounded-md border border-amber-500/40 bg-amber-500/10 px-1.5 py-0.5 text-[10px] font-medium text-amber-700 dark:text-amber-300">
								{option.badge}
							</span>
						) : null}
					</span>
					<span className="mt-1 block text-xs leading-5 text-muted-foreground">
						{option.description}
					</span>
				</button>
			))}
		</div>
	);
}
