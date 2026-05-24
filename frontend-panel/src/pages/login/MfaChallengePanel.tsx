import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

type Translate = (key: string, options?: Record<string, unknown>) => string;

interface MfaChallengePanelProps {
	code: string;
	error: string;
	expired: boolean;
	remainingSeconds: number;
	submitting: boolean;
	t: Translate;
	onBack: () => void;
	onCodeChange: (value: string) => void;
}

export function MfaChallengePanel({
	code,
	error,
	expired,
	remainingSeconds,
	submitting,
	t,
	onBack,
	onCodeChange,
}: MfaChallengePanelProps) {
	return (
		<div className="space-y-4 rounded-2xl border bg-muted/20 p-4 transition-[background-color,border-color] duration-200">
			<div className="flex items-start gap-3">
				<div className="rounded-xl bg-primary/10 p-2 text-primary">
					<Icon name="Shield" className="size-5" />
				</div>
				<div className="min-w-0 space-y-1">
					<p className="text-sm font-medium">{t("mfa_panel_title")}</p>
					<p className="text-sm text-muted-foreground">
						{expired
							? t("mfa_flow_expired")
							: t("mfa_flow_remaining", { seconds: remainingSeconds })}
					</p>
				</div>
			</div>

			<div className="space-y-1.5">
				<Label htmlFor="mfa-code" className="text-sm">
					{t("mfa_code_label")}
				</Label>
				<Input
					id="mfa-code"
					value={code}
					disabled={submitting || expired}
					autoComplete="one-time-code"
					autoFocus
					placeholder={t("mfa_code_placeholder")}
					className={
						error ? "border-destructive focus-visible:ring-destructive" : ""
					}
					onChange={(event) => onCodeChange(event.target.value)}
				/>
				{error ? <p className="text-xs text-destructive">{error}</p> : null}
			</div>

			<Button
				type="submit"
				className="h-10 w-full"
				disabled={submitting || expired || code.trim().length === 0}
			>
				{submitting ? (
					<Icon name="Spinner" className="mr-2 size-4 animate-spin" />
				) : (
					<Icon name="SignIn" className="mr-2 size-4" />
				)}
				{submitting ? t("mfa_verifying") : t("mfa_verify")}
			</Button>

			<Button
				type="button"
				variant="ghost"
				className="h-9 w-full"
				disabled={submitting}
				onClick={onBack}
			>
				<Icon name="ArrowLeft" className="mr-2 size-4" />
				{t("back_to_sign_in")}
			</Button>
		</div>
	);
}
