import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { emptyForm } from "@/components/admin/storagePolicyDialogShared";
import type { Translate } from "./StoragePolicyFieldTypes";
import { S3ConnectionFields } from "./StoragePolicyS3Fields";

const labels: Record<string, string> = {
	access_key: "Access key",
	bucket: "Bucket",
	s3_endpoint_hint: "S3 endpoint hint",
	s3_path_style: "Path-style addressing",
	s3_path_style_desc: "Use /bucket/key requests.",
	secret_key: "Secret key",
	endpoint: "Endpoint",
};

const t: Translate = (key) => labels[key] ?? key;

vi.mock("@/components/ui/input", () => ({
	Input: ({
		"aria-invalid": ariaInvalid,
		id,
		onBlur,
		onChange,
		placeholder,
		required,
		type,
		value,
	}: {
		"aria-invalid"?: boolean;
		id?: string;
		onBlur?: () => void;
		onChange?: (event: { target: { value: string } }) => void;
		placeholder?: string;
		required?: boolean;
		type?: string;
		value?: string;
	}) => (
		<input
			aria-invalid={ariaInvalid}
			id={id}
			onBlur={onBlur}
			onChange={(event) =>
				onChange?.({ target: { value: event.target.value } })
			}
			placeholder={placeholder}
			required={required}
			type={type}
			value={value}
		/>
	),
}));

vi.mock("@/components/ui/label", () => ({
	Label: ({
		children,
		htmlFor,
	}: {
		children: React.ReactNode;
		htmlFor?: string;
	}) => <label htmlFor={htmlFor}>{children}</label>,
}));

vi.mock("@/components/ui/switch", () => ({
	Switch: ({
		checked,
		id,
		onCheckedChange,
	}: {
		checked: boolean;
		id?: string;
		onCheckedChange?: (checked: boolean) => void;
	}) => (
		<button
			type="button"
			aria-label={`switch:${id ?? "toggle"}:${checked}`}
			onClick={() => onCheckedChange?.(!checked)}
		/>
	),
}));

function renderS3ConnectionFields(
	form: React.ComponentProps<typeof S3ConnectionFields>["form"],
	onFieldChange = vi.fn(),
) {
	render(
		<S3ConnectionFields
			bucketError={null}
			endpointValidationMessage={null}
			form={form}
			isCreateMode
			onFieldChange={onFieldChange}
			onSyncNormalizedS3Form={vi.fn()}
			t={t}
		/>,
	);
	return onFieldChange;
}

describe("S3ConnectionFields", () => {
	it("shows the path-style switch for generic S3 policies", () => {
		const onFieldChange = renderS3ConnectionFields({
			...emptyForm,
			driver_type: "s3",
			s3_path_style: true,
		});

		expect(screen.getByText("Path-style addressing")).toBeInTheDocument();
		expect(screen.getByText("Use /bucket/key requests.")).toBeInTheDocument();
		fireEvent.click(screen.getByLabelText("switch:s3_path_style:true"));
		expect(onFieldChange).toHaveBeenCalledWith("s3_path_style", false);
	});

	it("hides the path-style switch for Tencent COS policies", () => {
		renderS3ConnectionFields({
			...emptyForm,
			driver_type: "tencent_cos",
		});

		expect(screen.queryByText("Path-style addressing")).not.toBeInTheDocument();
	});
});
