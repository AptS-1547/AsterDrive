import { useTranslation } from "react-i18next";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { AdminExternalAuthProviderKindInfo } from "@/types/api";
import {
	type ExternalAuthProviderFieldChange,
	type ExternalAuthProviderFormData,
	STANDARD_CLAIMS,
} from "./shared";

interface ExternalAuthClaimFieldsProps {
	form: ExternalAuthProviderFormData;
	onFieldChange: ExternalAuthProviderFieldChange;
	selectedKind: AdminExternalAuthProviderKindInfo | null;
}

export function ExternalAuthClaimFields({
	form,
	onFieldChange,
	selectedKind,
}: ExternalAuthClaimFieldsProps) {
	const { t } = useTranslation("admin");

	return (
		<>
			<div className="space-y-2">
				<Label htmlFor="external-auth-provider-subject-claim">
					{t("external_auth_provider_subject_claim")}
				</Label>
				<Input
					id="external-auth-provider-subject-claim"
					value={form.subjectClaim}
					placeholder="sub"
					onChange={(event) =>
						onFieldChange("subjectClaim", event.target.value)
					}
				/>
				<p className="text-xs text-muted-foreground">
					{t("external_auth_provider_claim_default_hint", {
						claim: STANDARD_CLAIMS.subjectClaim,
					})}
				</p>
			</div>
			<div className="space-y-2">
				<Label htmlFor="external-auth-provider-username-claim">
					{t("external_auth_provider_username_claim")}
				</Label>
				<Input
					id="external-auth-provider-username-claim"
					value={form.usernameClaim}
					placeholder="preferred_username"
					onChange={(event) =>
						onFieldChange("usernameClaim", event.target.value)
					}
				/>
				<p className="text-xs text-muted-foreground">
					{t("external_auth_provider_claim_default_hint", {
						claim: STANDARD_CLAIMS.usernameClaim,
					})}
				</p>
			</div>
			<div className="space-y-2">
				<Label htmlFor="external-auth-provider-display-claim">
					{t("external_auth_provider_display_name_claim")}
				</Label>
				<Input
					id="external-auth-provider-display-claim"
					value={form.displayNameClaim}
					placeholder="name"
					onChange={(event) =>
						onFieldChange("displayNameClaim", event.target.value)
					}
				/>
				<p className="text-xs text-muted-foreground">
					{t("external_auth_provider_claim_default_hint", {
						claim: STANDARD_CLAIMS.displayNameClaim,
					})}
				</p>
			</div>
			<div className="space-y-2">
				<Label htmlFor="external-auth-provider-email-claim">
					{t("external_auth_provider_email_claim")}
				</Label>
				<Input
					id="external-auth-provider-email-claim"
					value={form.emailClaim}
					placeholder="email"
					onChange={(event) => onFieldChange("emailClaim", event.target.value)}
				/>
				<p className="text-xs text-muted-foreground">
					{t("external_auth_provider_claim_default_hint", {
						claim: STANDARD_CLAIMS.emailClaim,
					})}
				</p>
			</div>
			<div className="space-y-2">
				<Label htmlFor="external-auth-provider-groups-claim">
					{t("external_auth_provider_groups_claim")}
				</Label>
				<Input
					id="external-auth-provider-groups-claim"
					value={form.groupsClaim}
					placeholder="groups"
					onChange={(event) => onFieldChange("groupsClaim", event.target.value)}
				/>
				<p className="text-xs text-muted-foreground">
					{t("external_auth_provider_claim_default_hint", {
						claim: STANDARD_CLAIMS.groupsClaim,
					})}
				</p>
			</div>
			{selectedKind?.supports_email_verified_claim ? (
				<div className="space-y-2">
					<Label htmlFor="external-auth-provider-email-verified-claim">
						{t("external_auth_provider_email_verified_claim")}
					</Label>
					<Input
						id="external-auth-provider-email-verified-claim"
						value={form.emailVerifiedClaim}
						placeholder="email_verified"
						onChange={(event) =>
							onFieldChange("emailVerifiedClaim", event.target.value)
						}
					/>
					<p className="text-xs text-muted-foreground">
						{t("external_auth_provider_claim_default_hint", {
							claim: STANDARD_CLAIMS.emailVerifiedClaim,
						})}
					</p>
				</div>
			) : null}
			<div className="space-y-2">
				<Label htmlFor="external-auth-provider-avatar-claim">
					{t("external_auth_provider_avatar_url_claim")}
				</Label>
				<Input
					id="external-auth-provider-avatar-claim"
					value={form.avatarUrlClaim}
					placeholder="picture"
					onChange={(event) =>
						onFieldChange("avatarUrlClaim", event.target.value)
					}
				/>
				<p className="text-xs text-muted-foreground">
					{t("external_auth_provider_claim_default_hint", {
						claim: STANDARD_CLAIMS.avatarUrlClaim,
					})}
				</p>
			</div>
		</>
	);
}
