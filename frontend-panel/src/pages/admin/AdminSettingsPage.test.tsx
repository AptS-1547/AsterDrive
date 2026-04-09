import {
	act,
	createEvent,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AdminSettingsPage from "@/pages/admin/AdminSettingsPage";

const mockState = vi.hoisted(() => ({
	deleteConfig: vi.fn(),
	handleApiError: vi.fn(),
	listConfigs: vi.fn(),
	navigate: vi.fn(),
	schema: vi.fn(),
	sendTestEmail: vi.fn(),
	setConfig: vi.fn(),
	toastSuccess: vi.fn(),
}));

const translationMap: Record<string, string> = {
	settings_item_auth_access_token_ttl_secs_desc:
		"Access token lifetime in seconds",
	settings_item_auth_access_token_ttl_secs_label: "Access token lifetime",
	mail_send_test_email: "mail_send_test_email",
	mail_send_test_email_hint: "mail_send_test_email_hint",
	mail_test_email_dialog_desc: "mail_test_email_dialog_desc",
	mail_test_email_dialog_title: "mail_test_email_dialog_title",
	mail_test_email_recipient_label: "mail_test_email_recipient_label",
	mail_test_email_recipient_placeholder:
		"mail_test_email_recipient_placeholder",
	settings_save_hint:
		"更改会先暂存为草稿，确认无误后再统一保存，⌘/Ctrl + S 保存。",
};

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		t: (key: string, options?: Record<string, unknown>) => {
			if (key === "settings_save_notice")
				return `settings_save_notice:${options?.count}`;
			return translationMap[key] ?? key;
		},
	}),
}));

vi.mock("react-router-dom", () => ({
	useNavigate: () => mockState.navigate,
}));

vi.mock("@/stores/authStore", () => ({
	useAuthStore: (
		selector: (state: { user: { email: string } | null }) => unknown,
	) => selector({ user: { email: "admin@example.com" } }),
}));

vi.mock("sonner", () => ({
	toast: {
		success: (...args: unknown[]) => mockState.toastSuccess(...args),
	},
}));

vi.mock("@/components/common/EmptyState", () => ({
	EmptyState: ({ title }: { title: string }) => <div>{title}</div>,
}));

vi.mock("@/components/common/SkeletonTable", () => ({
	SkeletonTable: ({ columns, rows }: { columns: number; rows: number }) => (
		<div>{`skeleton:${columns}:${rows}`}</div>
	),
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
	}: {
		title: string;
		description: string;
	}) => (
		<div>
			<h1>{title}</h1>
			<p>{description}</p>
		</div>
	),
}));

vi.mock("@/components/layout/AdminPageShell", () => ({
	AdminPageShell: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
}));

vi.mock("@/components/ui/button", () => ({
	Button: ({
		children,
		className,
		disabled,
		onClick,
		type,
	}: {
		children: React.ReactNode;
		className?: string;
		disabled?: boolean;
		onClick?: () => void;
		type?: "button" | "submit";
	}) => (
		<button
			type={type ?? "button"}
			className={className}
			disabled={disabled}
			onClick={onClick}
		>
			{children}
		</button>
	),
	buttonVariants: ({
		className,
	}: {
		className?: string;
		variant?: string;
		size?: string;
	}) => className ?? "",
}));

vi.mock("@/components/ui/icon", () => ({
	Icon: ({ name }: { name: string }) => <span>{name}</span>,
}));

vi.mock("@/components/ui/input", () => ({
	Input: ({
		ariaInvalid,
		className,
		onChange,
		placeholder,
		type,
		value,
	}: {
		ariaInvalid?: boolean;
		className?: string;
		onChange?: (event: { target: { value: string } }) => void;
		placeholder?: string;
		type?: string;
		value?: string;
	}) => (
		<input
			aria-invalid={ariaInvalid}
			className={className}
			onChange={(event) =>
				onChange?.({ target: { value: event.target.value } })
			}
			placeholder={placeholder}
			type={type}
			value={value}
		/>
	),
}));

vi.mock("@/components/ui/dialog", () => ({
	Dialog: ({
		children,
		open,
	}: {
		children: React.ReactNode;
		open?: boolean;
	}) => (open ? <div>{children}</div> : null),
	DialogContent: ({ children }: { children: React.ReactNode }) => (
		<div role="dialog">{children}</div>
	),
	DialogDescription: ({ children }: { children: React.ReactNode }) => (
		<p>{children}</p>
	),
	DialogFooter: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	DialogHeader: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	DialogTitle: ({ children }: { children: React.ReactNode }) => (
		<h2>{children}</h2>
	),
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
			id={id}
			aria-label={`switch:${id ?? "config"}:${checked}`}
			onClick={() => onCheckedChange?.(!checked)}
		/>
	),
}));

vi.mock("@/components/ui/tabs", async () => {
	const React = await vi.importActual<typeof import("react")>("react");
	const TabsContext = React.createContext<{
		onValueChange?: (value: string) => void;
		value?: string;
	}>({});

	return {
		Tabs: ({
			children,
			onValueChange,
			value,
		}: {
			children: React.ReactNode;
			onValueChange?: (value: string) => void;
			value?: string;
		}) => (
			<TabsContext.Provider value={{ onValueChange, value }}>
				<div>{children}</div>
			</TabsContext.Provider>
		),
		TabsList: ({
			children,
			className,
		}: {
			children: React.ReactNode;
			className?: string;
		}) => <div className={className}>{children}</div>,
		TabsTrigger: ({
			children,
			className,
			value,
		}: {
			children: React.ReactNode;
			className?: string;
			value: string;
		}) => {
			const tabs = React.useContext(TabsContext);

			return (
				<button
					type="button"
					className={className}
					data-active={tabs.value === value ? "" : undefined}
					data-value={value}
					onClick={() => tabs.onValueChange?.(value)}
				>
					{children}
				</button>
			);
		},
		TabsContent: ({
			children,
			value,
		}: {
			children: React.ReactNode;
			value?: string;
		}) => {
			const tabs = React.useContext(TabsContext);
			return tabs.value === value ? <div>{children}</div> : null;
		},
	};
});

vi.mock("@/components/ui/dropdown-menu", () => ({
	DropdownMenu: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	DropdownMenuTrigger: ({
		children,
		render,
	}: {
		children?: React.ReactNode;
		render?: React.ReactNode;
	}) => <>{render ?? children}</>,
	DropdownMenuContent: () => null,
	DropdownMenuRadioGroup: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	DropdownMenuRadioItem: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
}));

vi.mock("@/hooks/useApiError", () => ({
	handleApiError: (...args: unknown[]) => mockState.handleApiError(...args),
}));

vi.mock("@/services/adminService", () => ({
	adminConfigService: {
		delete: (...args: unknown[]) => mockState.deleteConfig(...args),
		list: (...args: unknown[]) => mockState.listConfigs(...args),
		sendTestEmail: (...args: unknown[]) => mockState.sendTestEmail(...args),
		schema: (...args: unknown[]) => mockState.schema(...args),
		set: (...args: unknown[]) => mockState.setConfig(...args),
	},
}));

function createConfig(overrides: Record<string, unknown> = {}) {
	return {
		category: "storage",
		description: "desc",
		is_sensitive: false,
		key: "storage.enabled",
		requires_restart: false,
		source: "system",
		value: "true",
		value_type: "boolean",
		...overrides,
	};
}

function createSchemaItem(overrides: Record<string, unknown> = {}) {
	return {
		category: "storage",
		default_value: "true",
		description: "desc",
		description_i18n_key: undefined,
		is_sensitive: false,
		key: "storage.enabled",
		label_i18n_key: undefined,
		requires_restart: false,
		value_type: "boolean",
		...overrides,
	};
}

function getMockConfigCategory(key: string) {
	if (key.startsWith("auth")) return "auth";
	if (key.startsWith("custom")) return "custom";
	return "storage";
}

describe("AdminSettingsPage", () => {
	beforeEach(() => {
		mockState.deleteConfig.mockReset();
		mockState.handleApiError.mockReset();
		mockState.listConfigs.mockReset();
		mockState.navigate.mockReset();
		mockState.schema.mockReset();
		mockState.sendTestEmail.mockReset();
		mockState.setConfig.mockReset();
		mockState.toastSuccess.mockReset();

		Object.defineProperty(window, "innerWidth", {
			configurable: true,
			value: 1024,
			writable: true,
		});

		mockState.listConfigs.mockResolvedValue({
			items: [
				createConfig(),
				createConfig({
					category: "auth",
					description: "ttl desc",
					key: "auth_access_token_ttl_secs",
					value: "1200",
					value_type: "number",
				}),
				createConfig({
					category: "custom",
					description: "",
					key: "custom.theme",
					source: "custom",
					value: "ocean",
					value_type: "string",
				}),
			],
		});
		mockState.schema.mockResolvedValue([
			createSchemaItem(),
			createSchemaItem({
				category: "auth",
				default_value: "900",
				description: "ttl desc",
				description_i18n_key: "settings_item_auth_access_token_ttl_secs_desc",
				key: "auth_access_token_ttl_secs",
				label_i18n_key: "settings_item_auth_access_token_ttl_secs_label",
				value_type: "number",
			}),
		]);
		mockState.sendTestEmail.mockResolvedValue({
			message: "Test email sent to admin@example.com",
		});
		mockState.setConfig.mockImplementation((key: string, value: string) =>
			Promise.resolve(
				createConfig({
					category: getMockConfigCategory(key),
					key,
					source: key.startsWith("custom") ? "custom" : "system",
					value,
					value_type:
						key === "storage.enabled"
							? "boolean"
							: key === "auth_access_token_ttl_secs"
								? "number"
								: "string",
				}),
			),
		);
		mockState.deleteConfig.mockResolvedValue(undefined);
	});

	it("shows a loading state and then an empty state when there are no configs", async () => {
		mockState.listConfigs.mockResolvedValueOnce({ items: [] });
		mockState.schema.mockResolvedValueOnce([]);

		render(<AdminSettingsPage />);

		expect(screen.getByText("skeleton:4:8")).toBeInTheDocument();
		expect(await screen.findByText("no_config")).toBeInTheDocument();
	});

	it("hides duplicate category headings and descriptions on mobile", async () => {
		Object.defineProperty(window, "innerWidth", {
			configurable: true,
			value: 375,
			writable: true,
		});

		render(<AdminSettingsPage />);

		await screen.findByDisplayValue("1200");

		expect(
			screen.queryByRole("heading", { name: "settings_category_auth" }),
		).not.toBeInTheDocument();
		expect(
			screen.queryByText("settings_category_auth_desc"),
		).not.toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: /settings_category_auth/i }),
		).toBeInTheDocument();
	});

	it("keeps category descriptions in the desktop sidebar without repeating them in content", async () => {
		Object.defineProperty(window, "innerWidth", {
			configurable: true,
			value: 1440,
			writable: true,
		});

		render(<AdminSettingsPage section="auth" />);

		await screen.findByDisplayValue("1200");

		expect(screen.getAllByText("settings_category_auth_desc")).toHaveLength(1);
		expect(screen.getAllByText("Shield")).toHaveLength(1);
	});

	it("renders category tabs and only saves boolean changes from the bottom action", async () => {
		render(<AdminSettingsPage section="storage" />);

		await screen.findByRole("button", { name: /settings_category_auth/i });
		expect(
			screen.getByRole("button", { name: /settings_category_custom/i }),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "save_changes" }),
		).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "core:edit" }),
		).not.toBeInTheDocument();

		fireEvent.click(screen.getByLabelText("switch:storage.enabled:true"));

		expect(mockState.setConfig).not.toHaveBeenCalled();
		expect(
			await screen.findByText("settings_save_notice:1"),
		).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "save_changes" }));

		await waitFor(() => {
			expect(mockState.setConfig).toHaveBeenCalledWith(
				"storage.enabled",
				"false",
			);
		});
		expect(
			screen.getByLabelText("switch:storage.enabled:false"),
		).toBeInTheDocument();
		expect(mockState.listConfigs).toHaveBeenCalledTimes(1);
		expect(mockState.schema).toHaveBeenCalledTimes(1);
		expect(mockState.toastSuccess).toHaveBeenCalledWith("settings_saved");
	});

	it("opens the mail test dialog with the current user email and sends to the edited recipient", async () => {
		mockState.listConfigs.mockResolvedValueOnce({
			items: [
				createConfig({
					category: "mail",
					key: "mail_smtp_host",
					value: "smtp.example.com",
					value_type: "string",
				}),
			],
		});
		mockState.schema.mockResolvedValueOnce([
			createSchemaItem({
				category: "mail",
				key: "mail_smtp_host",
				value_type: "string",
			}),
		]);
		mockState.sendTestEmail.mockResolvedValueOnce({
			message: "Test email sent to deliver@example.com",
		});

		render(<AdminSettingsPage section="mail" />);

		fireEvent.click(
			await screen.findByRole("button", { name: /mail_send_test_email/i }),
		);

		const dialog = screen.getByRole("dialog");
		expect(
			within(dialog).getByDisplayValue("admin@example.com"),
		).toBeInTheDocument();

		fireEvent.change(within(dialog).getByDisplayValue("admin@example.com"), {
			target: { value: "deliver@example.com" },
		});
		fireEvent.click(
			within(dialog).getAllByRole("button", {
				name: /mail_send_test_email/i,
			})[0],
		);

		await waitFor(() => {
			expect(mockState.sendTestEmail).toHaveBeenCalledWith(
				"deliver@example.com",
			);
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith(
			"Test email sent to deliver@example.com",
		);
	});

	it("uses the shared underline accent for compact category tabs", async () => {
		render(<AdminSettingsPage />);

		const storageTab = await screen.findByRole("button", {
			name: /settings_category_storage/i,
		});

		expect(storageTab).toHaveClass("h-10", "rounded-none", "px-0");
		expect(storageTab).not.toHaveClass("after:hidden");

		const tabContent = storageTab.firstElementChild;
		expect(tabContent).not.toBeNull();
		expect(tabContent).not.toHaveClass("group-data-[active]:bg-muted/70");

		const compactTabList = storageTab.parentElement;
		expect(compactTabList).not.toBeNull();
		expect(compactTabList).toHaveClass(
			"overflow-hidden",
			"border-b",
			"border-border/40",
			"pb-2",
		);
		expect(compactTabList).not.toHaveClass("overflow-x-auto");

		const compactNav = compactTabList?.parentElement;
		expect(compactNav).not.toBeNull();
		expect(compactNav).toHaveClass("items-end");
	});

	it("adds a vertical divider for the desktop category sidebar", async () => {
		Object.defineProperty(window, "innerWidth", {
			configurable: true,
			value: 1440,
			writable: true,
		});

		render(<AdminSettingsPage />);

		const storageTab = await screen.findByRole("button", {
			name: /settings_category_storage/i,
		});

		const desktopNav = storageTab.parentElement?.parentElement;
		expect(desktopNav).not.toBeNull();
		expect(desktopNav).toHaveClass("border-r", "border-border/40");
	});

	it("navigates to the matching settings route when a tab is selected", async () => {
		Object.defineProperty(window, "innerWidth", {
			configurable: true,
			value: 1440,
			writable: true,
		});

		render(<AdminSettingsPage section="auth" />);

		const storageTab = await screen.findByRole("button", {
			name: /settings_category_storage/i,
		});

		fireEvent.click(storageTab);

		expect(mockState.navigate).toHaveBeenCalledWith("/admin/settings/storage", {
			viewTransition: true,
		});
	});

	it("redirects invalid sections to the first available settings tab", async () => {
		render(<AdminSettingsPage section={"invalid" as never} />);

		await screen.findByRole("button", { name: /settings_category_auth/i });

		await waitFor(() => {
			expect(mockState.navigate).toHaveBeenCalledWith("/admin/settings/auth", {
				replace: true,
			});
		});
	});

	it("renders site settings before the other categories when available", async () => {
		Object.defineProperty(window, "innerWidth", {
			configurable: true,
			value: 1440,
			writable: true,
		});

		mockState.listConfigs.mockResolvedValueOnce({
			items: [
				createConfig({
					category: "general",
					key: "branding_title",
					value: "AsterDrive",
					value_type: "string",
				}),
				createConfig({
					category: "auth",
					key: "auth_access_token_ttl_secs",
					value: "1200",
					value_type: "number",
				}),
			],
		});
		mockState.schema.mockResolvedValueOnce([
			createSchemaItem({
				category: "general",
				default_value: "AsterDrive",
				key: "branding_title",
				value_type: "string",
			}),
			createSchemaItem({
				category: "auth",
				default_value: "900",
				key: "auth_access_token_ttl_secs",
				value_type: "number",
			}),
		]);

		const { container } = render(<AdminSettingsPage section="general" />);

		await screen.findByDisplayValue("AsterDrive");

		const categoryButtons = Array.from(
			container.querySelectorAll("button[data-value]"),
		).map((button) => button.getAttribute("data-value"));

		expect(categoryButtons.slice(0, 2)).toEqual(["general", "auth"]);
	});

	it("edits non-boolean values inline and saves them with the shared save button", async () => {
		render(<AdminSettingsPage />);

		await screen.findByDisplayValue("1200");

		fireEvent.change(screen.getByDisplayValue("1200"), {
			target: { value: "1800" },
		});

		expect(
			await screen.findByText("settings_save_notice:1"),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "save_changes" }));

		await waitFor(() => {
			expect(mockState.setConfig).toHaveBeenCalledWith(
				"auth_access_token_ttl_secs",
				"1800",
			);
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith("settings_saved");
	});

	it("saves staged changes when Cmd+S is pressed from a focused input", async () => {
		render(<AdminSettingsPage />);

		const ttlInput = await screen.findByDisplayValue("1200");
		ttlInput.focus();

		fireEvent.change(ttlInput, {
			target: { value: "1800" },
		});

		expect(
			await screen.findByText("settings_save_notice:1"),
		).toBeInTheDocument();

		const saveEvent = createEvent.keyDown(ttlInput, {
			bubbles: true,
			cancelable: true,
			key: "s",
			metaKey: true,
		});

		fireEvent(ttlInput, saveEvent);

		expect(saveEvent.defaultPrevented).toBe(true);

		await waitFor(() => {
			expect(mockState.setConfig).toHaveBeenCalledWith(
				"auth_access_token_ttl_secs",
				"1800",
			);
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith("settings_saved");
	});

	it("shows the combined save hint when valid staged changes are present", async () => {
		render(<AdminSettingsPage />);

		await screen.findByDisplayValue("1200");

		fireEvent.change(screen.getByDisplayValue("1200"), {
			target: { value: "1800" },
		});

		expect(
			await screen.findByText(
				"更改会先暂存为草稿，确认无误后再统一保存，⌘/Ctrl + S 保存。",
			),
		).toBeInTheDocument();
	});

	it("renders the save actions in a floating bottom bar", async () => {
		render(<AdminSettingsPage />);

		await screen.findByDisplayValue("1200");

		fireEvent.change(screen.getByDisplayValue("1200"), {
			target: { value: "1800" },
		});

		const saveBar = await screen.findByTestId("settings-save-bar");

		expect(saveBar).toHaveClass("fixed", "bottom-0", "pointer-events-none");
		expect(saveBar).toHaveAttribute("aria-hidden", "false");
		expect(saveBar).toHaveTextContent("settings_save_notice:1");
	});

	it("reserves bottom space for settings content while the floating save bar is visible", async () => {
		render(<AdminSettingsPage />);

		await screen.findByDisplayValue("1200");

		fireEvent.change(screen.getByDisplayValue("1200"), {
			target: { value: "1800" },
		});

		await screen.findByTestId("settings-save-bar");

		await waitFor(() => {
			expect(screen.getByTestId("settings-content")).toHaveStyle({
				paddingBottom: "160px",
			});
		});
	});

	it("keeps the floating save bar mounted for the exit animation before unmounting", async () => {
		render(<AdminSettingsPage />);

		await screen.findByDisplayValue("1200");

		fireEvent.change(screen.getByDisplayValue("1200"), {
			target: { value: "1800" },
		});

		const saveBar = await screen.findByTestId("settings-save-bar");

		vi.useFakeTimers();
		fireEvent.click(screen.getByRole("button", { name: "undo_changes" }));

		expect(saveBar).toBeInTheDocument();
		expect(saveBar).toHaveAttribute("aria-hidden", "true");

		await act(async () => {
			await vi.advanceTimersByTimeAsync(140);
		});

		expect(screen.queryByTestId("settings-save-bar")).not.toBeInTheDocument();
		vi.useRealTimers();
	});

	it("renders translated system config metadata without exposing the raw config key", async () => {
		render(<AdminSettingsPage section="auth" />);

		await screen.findByDisplayValue("1200");

		expect(screen.getByText("Access token lifetime")).toBeInTheDocument();
		expect(
			screen.getByText("Access token lifetime in seconds"),
		).toBeInTheDocument();
		expect(
			screen.queryByText("auth_access_token_ttl_secs"),
		).not.toBeInTheDocument();
		expect(screen.queryByText("ttl desc")).not.toBeInTheDocument();
	});

	it("shows a favicon asset preview next to the branding URL field", async () => {
		mockState.listConfigs.mockResolvedValueOnce({
			items: [
				createConfig({
					category: "general",
					description: "favicon desc",
					key: "branding_favicon_url",
					value: "/branding/favicon.svg",
					value_type: "string",
				}),
			],
		});
		mockState.schema.mockResolvedValueOnce([
			createSchemaItem({
				category: "general",
				default_value: "/favicon.svg",
				description: "favicon desc",
				key: "branding_favicon_url",
				value_type: "string",
			}),
		]);

		render(<AdminSettingsPage section="general" />);

		expect(
			await screen.findByDisplayValue("/branding/favicon.svg"),
		).toBeInTheDocument();
		expect(screen.getByTestId("branding-asset-preview")).toBeInTheDocument();
		expect(screen.getByTestId("branding-asset-preview-image")).toHaveAttribute(
			"src",
			"/branding/favicon.svg",
		);
		expect(screen.getByLabelText("/branding/favicon.svg")).toBeInTheDocument();
	});

	it("debounces favicon asset preview updates while typing", async () => {
		mockState.listConfigs.mockResolvedValueOnce({
			items: [
				createConfig({
					category: "general",
					key: "branding_favicon_url",
					value: "/branding/favicon.svg",
					value_type: "string",
				}),
			],
		});
		mockState.schema.mockResolvedValueOnce([
			createSchemaItem({
				category: "general",
				default_value: "/favicon.svg",
				key: "branding_favicon_url",
				value_type: "string",
			}),
		]);

		render(<AdminSettingsPage section="general" />);

		const input = await screen.findByDisplayValue("/branding/favicon.svg");
		expect(screen.getByLabelText("/branding/favicon.svg")).toBeInTheDocument();

		vi.useFakeTimers();
		fireEvent.change(input, {
			target: { value: "/branding/next.svg" },
		});

		expect(
			screen.queryByLabelText("/branding/next.svg"),
		).not.toBeInTheDocument();
		expect(screen.getByLabelText("/branding/favicon.svg")).toBeInTheDocument();

		await act(async () => {
			await vi.advanceTimersByTimeAsync(300);
		});

		expect(screen.getByLabelText("/branding/next.svg")).toBeInTheDocument();
	});

	it("discards draft changes without sending any requests", async () => {
		render(<AdminSettingsPage />);

		await screen.findByDisplayValue("1200");

		fireEvent.change(screen.getByDisplayValue("1200"), {
			target: { value: "1800" },
		});

		expect(
			await screen.findByText("settings_save_notice:1"),
		).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "undo_changes" }));

		await waitFor(() => {
			expect(
				screen.queryByRole("button", { name: "save_changes" }),
			).not.toBeInTheDocument();
		});
		expect(screen.getByDisplayValue("1200")).toBeInTheDocument();
		expect(mockState.setConfig).not.toHaveBeenCalled();
		expect(mockState.deleteConfig).not.toHaveBeenCalled();
	});

	it("stages custom config creation and deletion until the shared save button is clicked", async () => {
		render(<AdminSettingsPage section="custom" />);

		await screen.findByDisplayValue("ocean");

		fireEvent.click(screen.getByRole("button", { name: "core:delete" }));
		fireEvent.click(screen.getByRole("button", { name: /add_custom_config/i }));

		fireEvent.change(
			screen.getByPlaceholderText("custom_config_key_placeholder"),
			{
				target: { value: "custom.accent" },
			},
		);
		fireEvent.change(
			screen.getAllByPlaceholderText("config_value").slice(-1)[0],
			{
				target: { value: "sunset" },
			},
		);

		expect(
			await screen.findByText("settings_save_notice:2"),
		).toBeInTheDocument();

		fireEvent.click(screen.getByRole("button", { name: "save_changes" }));

		await waitFor(() => {
			expect(mockState.deleteConfig).toHaveBeenCalledWith("custom.theme");
			expect(mockState.setConfig).toHaveBeenCalledWith(
				"custom.accent",
				"sunset",
			);
		});
		expect(mockState.toastSuccess).toHaveBeenCalledWith("settings_saved");
	});
});
