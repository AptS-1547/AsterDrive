import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AdminPoliciesPage from "@/pages/admin/AdminPoliciesPage";

const mockState = vi.hoisted(() => ({
	create: vi.fn(),
	deletePolicy: vi.fn(),
	handleApiError: vi.fn(),
	items: [] as Array<Record<string, unknown>>,
	loading: false,
	testConnection: vi.fn(),
	testParams: vi.fn(),
	toastSuccess: vi.fn(),
	update: vi.fn(),
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string) => {
			switch (key) {
				case "driver_type_local":
					return "Local";
				case "driver_type_s3":
					return "S3";
				case "access_key":
					return "Access Key";
				case "secret_key":
					return "Secret Key";
				default:
					return key;
			}
		},
	}),
	initReactI18next: {
		type: "3rdParty",
		init: () => undefined,
	},
}));

vi.mock("sonner", () => ({
	toast: {
		success: (...args: unknown[]) => mockState.toastSuccess(...args),
	},
}));

vi.mock("@/components/common/AdminTableList", () => ({
	AdminTableList: ({
		items,
		loading,
		emptyTitle,
		emptyDescription,
		headerRow,
		renderRow,
	}: {
		items: unknown[];
		loading: boolean;
		emptyTitle: string;
		emptyDescription: string;
		headerRow: React.ReactNode;
		renderRow: (item: never) => React.ReactNode;
	}) =>
		loading ? (
			<div>loading</div>
		) : items.length === 0 ? (
			<div>{`${emptyTitle}:${emptyDescription}`}</div>
		) : (
			<div>
				{headerRow}
				{items.map((item) => (
					<div key={String((item as { id: number }).id)}>
						{renderRow(item as never)}
					</div>
				))}
			</div>
		),
}));

vi.mock("@/components/common/ConfirmDialog", () => ({
	ConfirmDialog: ({
		open,
		title,
		description,
		confirmLabel,
		onConfirm,
	}: {
		open: boolean;
		title: string;
		description?: string;
		confirmLabel?: string;
		onConfirm: () => void;
	}) =>
		open ? (
			<div>
				<div>{title}</div>
				<div>{description}</div>
				<button type="button" onClick={onConfirm}>
					{confirmLabel}
				</button>
			</div>
		) : null,
}));

vi.mock("@/components/layout/AdminLayout", () => ({
	AdminLayout: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
}));

vi.mock("@/components/layout/AdminPageHeader", () => ({
	AdminPageHeader: ({
		title,
		description,
		actions,
	}: {
		title: string;
		description: string;
		actions?: React.ReactNode;
	}) => (
		<div>
			<h1>{title}</h1>
			<p>{description}</p>
			<div>{actions}</div>
		</div>
	),
}));

vi.mock("@/components/layout/AdminPageShell", () => ({
	AdminPageShell: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
}));

vi.mock("@/components/ui/badge", () => ({
	Badge: ({ children }: { children: React.ReactNode }) => (
		<span>{children}</span>
	),
}));

vi.mock("@/components/ui/button", () => ({
	Button: ({
		children,
		className,
		disabled,
		onClick,
		type,
		variant,
	}: {
		children: React.ReactNode;
		className?: string;
		disabled?: boolean;
		onClick?: () => void;
		type?: "button" | "submit";
		variant?: string;
	}) => (
		<button
			type={type ?? "button"}
			className={className}
			data-variant={variant}
			disabled={disabled}
			onClick={onClick}
		>
			{children}
		</button>
	),
}));

vi.mock("@/components/ui/dialog", () => ({
	Dialog: ({ children, open }: { children: React.ReactNode; open: boolean }) =>
		open ? <div>{children}</div> : null,
	DialogContent: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	DialogHeader: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	DialogTitle: ({ children }: { children: React.ReactNode }) => (
		<h2>{children}</h2>
	),
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => <span>{name}</span>,
}));

vi.mock("@/components/ui/input", () => ({
	Input: ({
		id,
		onChange,
		placeholder,
		required,
		type,
		value,
	}: {
		id?: string;
		onChange?: (event: { target: { value: string } }) => void;
		placeholder?: string;
		required?: boolean;
		type?: string;
		value?: string;
	}) => (
		<input
			id={id}
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

vi.mock("@/components/ui/select", () => {
	const { createContext, useContext } =
		require("react") as typeof import("react");

	const SelectContext = createContext<{
		onValueChange?: (value: string) => void;
		disabled?: boolean;
	}>({});

	return {
		Select: ({
			children,
			disabled,
			onValueChange,
		}: {
			children: React.ReactNode;
			disabled?: boolean;
			onValueChange?: (value: string) => void;
		}) => (
			<SelectContext.Provider value={{ disabled, onValueChange }}>
				<div>{children}</div>
			</SelectContext.Provider>
		),
		SelectContent: ({ children }: { children: React.ReactNode }) => (
			<div>{children}</div>
		),
		SelectItem: ({
			children,
			value,
		}: {
			children: React.ReactNode;
			value: string;
		}) => {
			const context = useContext(SelectContext);

			return (
				<button
					type="button"
					aria-label={`select-item:${value}`}
					disabled={context.disabled}
					onClick={() => context.onValueChange?.(value)}
				>
					{children}
				</button>
			);
		},
		SelectTrigger: ({
			children,
			className,
		}: {
			children: React.ReactNode;
			className?: string;
		}) => <div className={className}>{children}</div>,
		SelectValue: () => <span>select-value</span>,
	};
});

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

vi.mock("@/components/ui/table", () => ({
	TableCell: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	TableHead: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	TableHeader: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	TableRow: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
}));

vi.mock("@/hooks/useApiError", () => ({
	handleApiError: (...args: unknown[]) => mockState.handleApiError(...args),
}));

vi.mock("@/hooks/useApiList", () => ({
	useApiList: () => {
		const [items, setItems] = useState(mockState.items);
		return { items, loading: mockState.loading, setItems };
	},
}));

vi.mock("@/services/adminService", () => ({
	adminPolicyService: {
		create: (...args: unknown[]) => mockState.create(...args),
		delete: (...args: unknown[]) => mockState.deletePolicy(...args),
		list: vi.fn(),
		testConnection: (...args: unknown[]) => mockState.testConnection(...args),
		testParams: (...args: unknown[]) => mockState.testParams(...args),
		update: (...args: unknown[]) => mockState.update(...args),
	},
}));

function createPolicy(overrides: Record<string, unknown> = {}) {
	return {
		allowed_types: "",
		base_path: "",
		bucket: "",
		chunk_size: 5 * 1024 * 1024,
		created_at: "2026-03-28T00:00:00Z",
		driver_type: "local",
		endpoint: "",
		id: 1,
		is_default: false,
		max_file_size: 0,
		name: "Local Policy",
		options: "{}",
		updated_at: "2026-03-28T00:00:00Z",
		...overrides,
	};
}

describe("AdminPoliciesPage", () => {
	beforeEach(() => {
		mockState.create.mockReset();
		mockState.deletePolicy.mockReset();
		mockState.handleApiError.mockReset();
		mockState.items = [];
		mockState.loading = false;
		mockState.testConnection.mockReset();
		mockState.testParams.mockReset();
		mockState.toastSuccess.mockReset();
		mockState.update.mockReset();

		mockState.create.mockImplementation(async (payload) =>
			createPolicy({
				...(payload as Record<string, unknown>),
				id: 99,
			}),
		);
		mockState.deletePolicy.mockResolvedValue(undefined);
		mockState.testConnection.mockResolvedValue(undefined);
		mockState.testParams.mockResolvedValue(undefined);
		mockState.update.mockImplementation(async (id, payload) =>
			createPolicy({
				...(payload as Record<string, unknown>),
				driver_type: "s3",
				id,
			}),
		);
	});

	it("renders local and s3 rows, including default and local fallback path states", () => {
		mockState.items = [
			createPolicy({
				id: 1,
				name: "Default Local",
				is_default: true,
			}),
			createPolicy({
				id: 2,
				name: "Archive S3",
				driver_type: "s3",
				endpoint: "https://s3.example.com",
				bucket: "archive",
			}),
		];

		render(<AdminPoliciesPage />);

		expect(screen.getByText("policies")).toBeInTheDocument();
		expect(screen.getByText("policies_intro")).toBeInTheDocument();
		expect(screen.getByText("Default Local")).toBeInTheDocument();
		expect(screen.getByText("Archive S3")).toBeInTheDocument();
		expect(screen.getByText("./data")).toBeInTheDocument();
		expect(screen.getByText("https://s3.example.com")).toBeInTheDocument();
		expect(screen.getByText("archive")).toBeInTheDocument();
		expect(screen.getAllByText("is_default")).toHaveLength(2);
		expect(screen.getByText("Local")).toBeInTheDocument();
		expect(screen.getByText("S3")).toBeInTheDocument();
	});

	it("tests create params and creates a new local policy", async () => {
		render(<AdminPoliciesPage />);

		fireEvent.click(screen.getByRole("button", { name: /new_policy/i }));

		fireEvent.change(screen.getByLabelText("core:name"), {
			target: { value: "Primary Local" },
		});
		fireEvent.change(screen.getByLabelText("base_path"), {
			target: { value: "/srv/data" },
		});
		fireEvent.change(screen.getByLabelText("max_file_size (bytes)"), {
			target: { value: "2048" },
		});
		fireEvent.change(screen.getByLabelText("chunk_size"), {
			target: { value: "8" },
		});
		fireEvent.click(
			screen.getByRole("button", { name: "switch:is_default:false" }),
		);

		fireEvent.click(screen.getByRole("button", { name: /test_connection/i }));

		await waitFor(() => {
			expect(mockState.testParams).toHaveBeenCalledWith({
				access_key: undefined,
				base_path: "/srv/data",
				bucket: undefined,
				driver_type: "local",
				endpoint: undefined,
				secret_key: undefined,
			});
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith("connection_success");

		fireEvent.click(screen.getByRole("button", { name: /core:create/i }));

		await waitFor(() => {
			expect(mockState.create).toHaveBeenCalledWith({
				access_key: "",
				base_path: "/srv/data",
				bucket: "",
				chunk_size: 8 * 1024 * 1024,
				driver_type: "local",
				endpoint: "",
				is_default: true,
				max_file_size: 2048,
				name: "Primary Local",
				options: JSON.stringify({}),
				secret_key: "",
			});
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith("policy_created");
	});

	it("tests changed s3 params and updates without sending blank secrets", async () => {
		mockState.items = [
			createPolicy({
				id: 7,
				name: "Archive S3",
				driver_type: "s3",
				endpoint: "https://s3.example.com",
				bucket: "archive",
				base_path: "tenant-a",
				max_file_size: 4096,
				options: JSON.stringify({ s3_upload_strategy: "presigned" }),
			}),
		];

		render(<AdminPoliciesPage />);

		fireEvent.click(screen.getByRole("button", { name: "PencilSimple" }));

		expect(screen.getByDisplayValue("Archive S3")).toBeInTheDocument();
		expect(screen.getByDisplayValue("tenant-a")).toBeInTheDocument();
		expect(screen.getByDisplayValue("4096")).toBeInTheDocument();
		expect(screen.getByDisplayValue("5")).toBeInTheDocument();

		fireEvent.change(screen.getByLabelText("core:name"), {
			target: { value: "Archive S3 Updated" },
		});
		fireEvent.change(screen.getByLabelText("Access Key"), {
			target: { value: "NEWKEY" },
		});
		fireEvent.click(
			screen.getByRole("button", { name: "select-item:proxy_tempfile" }),
		);
		fireEvent.click(screen.getByRole("button", { name: /test_connection/i }));

		await waitFor(() => {
			expect(mockState.testParams).toHaveBeenCalledWith({
				access_key: "NEWKEY",
				base_path: "tenant-a",
				bucket: "archive",
				driver_type: "s3",
				endpoint: "https://s3.example.com",
				secret_key: undefined,
			});
		});
		expect(mockState.testConnection).not.toHaveBeenCalled();

		fireEvent.click(screen.getByRole("button", { name: /save_changes/i }));

		await waitFor(() => {
			expect(mockState.update).toHaveBeenCalledTimes(1);
		});
		expect(mockState.testParams).toHaveBeenCalledTimes(1);

		const [, payload] = mockState.update.mock.calls[0] as [
			number,
			Record<string, unknown>,
		];
		expect(mockState.update).toHaveBeenCalledWith(
			7,
			expect.objectContaining({
				base_path: "tenant-a",
				bucket: "archive",
				chunk_size: 5 * 1024 * 1024,
				endpoint: "https://s3.example.com",
				is_default: false,
				max_file_size: 4096,
				name: "Archive S3 Updated",
				options: JSON.stringify({ s3_upload_strategy: "proxy_tempfile" }),
			}),
		);
		expect(payload).toHaveProperty("access_key", "NEWKEY");
		expect(payload).not.toHaveProperty("secret_key");
		expect(mockState.toastSuccess).toHaveBeenCalledWith("policy_updated");
	});

	it("displays legacy presigned_upload true as presigned strategy", async () => {
		mockState.items = [
			createPolicy({
				id: 10,
				name: "Legacy Presigned S3",
				driver_type: "s3",
				endpoint: "https://s3.example.com",
				bucket: "legacy-bucket",
				base_path: "legacy-path",
				options: JSON.stringify({ presigned_upload: true }),
			}),
		];

		render(<AdminPoliciesPage />);

		fireEvent.click(screen.getByRole("button", { name: "PencilSimple" }));

		expect(screen.getByDisplayValue("Legacy Presigned S3")).toBeInTheDocument();
		expect(screen.getByDisplayValue("legacy-bucket")).toBeInTheDocument();
		expect(screen.getByDisplayValue("legacy-path")).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "select-item:presigned" }),
		).toBeInTheDocument();

		fireEvent.change(screen.getByLabelText("Access Key"), {
			target: { value: "LEGACYKEY" },
		});
		fireEvent.click(screen.getByRole("button", { name: /test_connection/i }));

		await waitFor(() => {
			expect(mockState.testParams).toHaveBeenCalledWith({
				access_key: "LEGACYKEY",
				base_path: "legacy-path",
				bucket: "legacy-bucket",
				driver_type: "s3",
				endpoint: "https://s3.example.com",
				secret_key: undefined,
			});
		});

		fireEvent.click(screen.getByRole("button", { name: /save_changes/i }));

		await waitFor(() => {
			expect(mockState.update).toHaveBeenCalledWith(
				10,
				expect.objectContaining({
					options: JSON.stringify({ s3_upload_strategy: "presigned" }),
				}),
			);
		});

		const [, payload] = mockState.update.mock.calls[0] as [
			number,
			Record<string, unknown>,
		];
		expect(payload).toHaveProperty("access_key", "LEGACYKEY");
		expect(payload).not.toHaveProperty("secret_key");
		expect(mockState.toastSuccess).toHaveBeenCalledWith("policy_updated");
	});

	it("tests relay_stream params and updates s3 policy without blank secrets", async () => {
		mockState.items = [
			createPolicy({
				id: 9,
				name: "Relay S3",
				driver_type: "s3",
				endpoint: "https://s3.example.com",
				bucket: "relay-bucket",
				base_path: "tenant-relay",
				max_file_size: 4096,
				options: JSON.stringify({ s3_upload_strategy: "proxy_tempfile" }),
			}),
		];

		render(<AdminPoliciesPage />);

		fireEvent.click(screen.getByRole("button", { name: "PencilSimple" }));

		expect(screen.getByDisplayValue("Relay S3")).toBeInTheDocument();
		expect(screen.getByDisplayValue("tenant-relay")).toBeInTheDocument();

		fireEvent.change(screen.getByLabelText("Access Key"), {
			target: { value: "NEWKEY" },
		});
		fireEvent.click(
			screen.getByRole("button", { name: "select-item:relay_stream" }),
		);
		fireEvent.click(screen.getByRole("button", { name: /test_connection/i }));

		await waitFor(() => {
			expect(mockState.testParams).toHaveBeenCalledWith({
				access_key: "NEWKEY",
				base_path: "tenant-relay",
				bucket: "relay-bucket",
				driver_type: "s3",
				endpoint: "https://s3.example.com",
				secret_key: undefined,
			});
		});

		fireEvent.click(screen.getByRole("button", { name: /save_changes/i }));

		await waitFor(() => {
			expect(mockState.update).toHaveBeenCalledTimes(1);
		});

		const [, payload] = mockState.update.mock.calls[0] as [
			number,
			Record<string, unknown>,
		];
		expect(mockState.update).toHaveBeenCalledWith(
			9,
			expect.objectContaining({
				access_key: "NEWKEY",
				base_path: "tenant-relay",
				bucket: "relay-bucket",
				chunk_size: 5 * 1024 * 1024,
				endpoint: "https://s3.example.com",
				is_default: false,
				max_file_size: 4096,
				name: "Relay S3",
				options: JSON.stringify({ s3_upload_strategy: "relay_stream" }),
			}),
		);
		expect(payload).not.toHaveProperty("secret_key");
		expect(mockState.toastSuccess).toHaveBeenCalledWith("policy_updated");
	});

	it("preserves zero-valued policy limits when opening the edit dialog", () => {
		mockState.items = [
			createPolicy({
				id: 8,
				name: "Direct Put S3",
				driver_type: "s3",
				endpoint: "https://s3.example.com",
				bucket: "direct-put",
				max_file_size: 0,
				chunk_size: 0,
				options: JSON.stringify({ s3_upload_strategy: "presigned" }),
			}),
		];

		render(<AdminPoliciesPage />);

		fireEvent.click(screen.getByRole("button", { name: "PencilSimple" }));

		expect(screen.getByDisplayValue("Direct Put S3")).toBeInTheDocument();
		expect(screen.getAllByDisplayValue("0")).toHaveLength(2);
		expect(screen.queryByDisplayValue("5")).not.toBeInTheDocument();
	});

	it("asks for confirmation before force-saving a failing s3 create", async () => {
		mockState.testParams.mockRejectedValueOnce(new Error("bad s3 config"));

		render(<AdminPoliciesPage />);

		fireEvent.click(screen.getByRole("button", { name: /new_policy/i }));
		fireEvent.click(screen.getByRole("button", { name: "select-item:s3" }));

		fireEvent.change(screen.getByLabelText("core:name"), {
			target: { value: "Broken S3" },
		});
		fireEvent.change(screen.getByLabelText("endpoint"), {
			target: { value: "https://s3.example.com" },
		});
		fireEvent.change(screen.getByLabelText("bucket"), {
			target: { value: "broken-bucket" },
		});

		fireEvent.click(screen.getByRole("button", { name: /core:create/i }));

		await waitFor(() => {
			expect(mockState.testParams).toHaveBeenCalledWith({
				access_key: undefined,
				base_path: undefined,
				bucket: "broken-bucket",
				driver_type: "s3",
				endpoint: "https://s3.example.com",
				secret_key: undefined,
			});
		});
		expect(mockState.create).not.toHaveBeenCalled();
		expect(mockState.handleApiError).not.toHaveBeenCalled();
		expect(screen.getByText("connection_test_failed")).toBeInTheDocument();
		expect(
			screen.getByText("policy_test_failed_confirm_desc"),
		).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "save_anyway" }));

		await waitFor(() => {
			expect(mockState.create).toHaveBeenCalledWith({
				access_key: "",
				base_path: "",
				bucket: "broken-bucket",
				chunk_size: 5 * 1024 * 1024,
				driver_type: "s3",
				endpoint: "https://s3.example.com",
				is_default: false,
				max_file_size: undefined,
				name: "Broken S3",
				options: JSON.stringify({ s3_upload_strategy: "proxy_tempfile" }),
				secret_key: "",
			});
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith("policy_created");
	});

	it("confirms deletion and removes the policy row", async () => {
		mockState.items = [
			createPolicy({
				id: 8,
				name: "Remove Me",
			}),
		];

		render(<AdminPoliciesPage />);

		fireEvent.click(screen.getByRole("button", { name: "Trash" }));

		expect(screen.getByText('delete_policy "Remove Me"?')).toBeInTheDocument();
		expect(screen.getByText("delete_policy_desc")).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "core:delete" }));

		await waitFor(() => {
			expect(mockState.deletePolicy).toHaveBeenCalledWith(8);
		});
		await waitFor(() => {
			expect(screen.queryByText("Remove Me")).not.toBeInTheDocument();
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith("policy_deleted");
	});
});
