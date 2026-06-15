import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { ADMIN_CONTROL_HEIGHT_CLASS } from "@/lib/constants";
import type { OneDriveAccountMode } from "@/types/api";
import {
	getDefaultTenant,
	getTenantMode,
	ONE_DRIVE_AUTO_TENANT_MODE,
} from "./onedriveFieldUtils";
import type { SharedFieldProps, Translate } from "./StoragePolicyFieldTypes";

export function OneDriveTargetFields({
	accountModeOptions,
	form,
	onFieldChange,
	t,
}: SharedFieldProps & {
	accountModeOptions: Array<{
		label: string;
		value: OneDriveAccountMode;
	}>;
	t: Translate;
}) {
	return (
		<>
			<div className="space-y-2">
				<Label htmlFor="onedrive_account_mode">
					{t("onedrive_account_mode")}
				</Label>
				<Select
					items={accountModeOptions}
					value={form.onedrive_account_mode}
					onValueChange={(value) => {
						const nextMode = (value ?? "work_or_school") as OneDriveAccountMode;
						const tenantMode = getTenantMode(form);
						onFieldChange("onedrive_account_mode", nextMode);
						if (tenantMode === ONE_DRIVE_AUTO_TENANT_MODE) {
							onFieldChange("onedrive_tenant", getDefaultTenant(nextMode));
						}
					}}
				>
					<SelectTrigger id="onedrive_account_mode">
						<SelectValue />
					</SelectTrigger>
					<SelectContent>
						{accountModeOptions.map((option) => (
							<SelectItem key={option.value} value={option.value}>
								{option.label}
							</SelectItem>
						))}
					</SelectContent>
				</Select>
				<p className="text-xs leading-5 text-muted-foreground">
					{t("onedrive_account_mode_desc")}
				</p>
			</div>
			<div className="space-y-2">
				<Label htmlFor="onedrive_drive_id">{t("onedrive_drive_id")}</Label>
				<Input
					id="onedrive_drive_id"
					value={form.onedrive_drive_id}
					onChange={(event) =>
						onFieldChange("onedrive_drive_id", event.target.value)
					}
					className={ADMIN_CONTROL_HEIGHT_CLASS}
					placeholder={t("onedrive_drive_id_placeholder")}
				/>
				<p className="text-xs leading-5 text-muted-foreground">
					{t("onedrive_drive_id_desc")}
				</p>
			</div>

			<div className="space-y-2">
				<Label htmlFor="onedrive_root_item_id">
					{t("onedrive_root_item_id")}
				</Label>
				<Input
					id="onedrive_root_item_id"
					value={form.onedrive_root_item_id || "root"}
					onChange={(event) =>
						onFieldChange("onedrive_root_item_id", event.target.value)
					}
					className={ADMIN_CONTROL_HEIGHT_CLASS}
					placeholder="root"
				/>
				<p className="text-xs leading-5 text-muted-foreground">
					{t("onedrive_root_item_id_desc")}
				</p>
			</div>

			{form.onedrive_account_mode === "sharepoint_site" ? (
				<div className="space-y-2">
					<Label htmlFor="onedrive_site_id">{t("onedrive_site_id")}</Label>
					<Input
						id="onedrive_site_id"
						value={form.onedrive_site_id}
						onChange={(event) =>
							onFieldChange("onedrive_site_id", event.target.value)
						}
						className={ADMIN_CONTROL_HEIGHT_CLASS}
						placeholder="contoso.sharepoint.com,site-id,web-id"
					/>
					<p className="text-xs leading-5 text-muted-foreground">
						{t("onedrive_site_id_desc")}
					</p>
				</div>
			) : form.onedrive_account_mode === "group_drive" ? (
				<div className="space-y-2">
					<Label htmlFor="onedrive_group_id">{t("onedrive_group_id")}</Label>
					<Input
						id="onedrive_group_id"
						value={form.onedrive_group_id}
						onChange={(event) =>
							onFieldChange("onedrive_group_id", event.target.value)
						}
						className={ADMIN_CONTROL_HEIGHT_CLASS}
						placeholder="00000000-0000-0000-0000-000000000000"
					/>
					<p className="text-xs leading-5 text-muted-foreground">
						{t("onedrive_group_id_desc")}
					</p>
				</div>
			) : null}
		</>
	);
}
