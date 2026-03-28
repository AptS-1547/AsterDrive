import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createContext, useContext } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SettingsPage from "@/pages/SettingsPage";

const mockState = vi.hoisted(() => ({
	authService: {
		updateProfile: vi.fn(),
		setAvatarSource: vi.fn(),
		uploadAvatar: vi.fn(),
	},
	authStore: {
		refreshUser: vi.fn(),
		user: {
			email: "alice@example.com",
			id: 1,
			profile: {
				display_name: null,
				avatar: {
					source: "none",
					url_512: null,
					url_1024: null,
					version: 0,
				},
			},
			role: "user",
			status: "active",
			storage_quota: 0,
			storage_used: 0,
			username: "alice",
		},
	},
	changeLanguage: vi.fn(),
	fileStore: {
		setViewMode: vi.fn(),
		viewMode: "list" as "list" | "grid",
	},
	navigate: vi.fn(),
	preferenceSync: vi.fn(),
	themeStore: {
		mode: "dark" as "light" | "dark" | "system",
		setMode: vi.fn(),
	},
	translationLanguage: "zh-CN",
}));

vi.mock("react-i18next", () => ({
	useTranslation: () => ({
		i18n: {
			changeLanguage: mockState.changeLanguage,
			language: mockState.translationLanguage,
		},
		t: (key: string) => key,
	}),
}));

vi.mock("react-router-dom", () => ({
	useNavigate: () => mockState.navigate,
}));

vi.mock("@/components/common/ColorPresetPicker", () => ({
	ColorPresetPicker: () => <div>color-preset-picker</div>,
}));

vi.mock("@/components/common/UserAvatarImage", () => ({
	UserAvatarImage: ({ name }: { name: string }) => (
		<div>{`avatar:${name}`}</div>
	),
}));

vi.mock("@/components/common/SettingsScaffold", () => ({
	SettingsPageIntro: ({
		title,
		description,
	}: {
		title: string;
		description?: string;
	}) => (
		<div>
			<h1>{title}</h1>
			<p>{description}</p>
		</div>
	),
	SettingsSection: ({
		title,
		description,
		children,
	}: {
		title: string;
		description?: string;
		children: React.ReactNode;
	}) => (
		<section>
			<h2>{title}</h2>
			<p>{description}</p>
			{children}
		</section>
	),
	SettingsRow: ({
		label,
		description,
		children,
	}: {
		label: string;
		description?: string;
		children: React.ReactNode;
	}) => (
		<div>
			<div>{label}</div>
			<div>{description}</div>
			{children}
		</div>
	),
	SettingsChoiceGroup: ({
		options,
		value,
		onChange,
	}: {
		options: Array<{ label: string; value: string }>;
		value: string;
		onChange: (value: never) => void;
	}) => (
		<div data-testid="choice-group" data-value={value}>
			{options.map((option) => (
				<button
					key={option.value}
					type="button"
					onClick={() => onChange(option.value as never)}
				>
					{option.label}
				</button>
			))}
		</div>
	),
}));

vi.mock("@/components/layout/AppLayout", () => ({
	AppLayout: ({ children }: { children: React.ReactNode }) => (
		<div data-testid="app-layout">{children}</div>
	),
}));

const TabsContext = createContext<{
	onValueChange?: (value: string) => void;
	value: string;
}>({
	value: "",
});

vi.mock("@/components/ui/tabs", () => ({
	Tabs: ({
		children,
		defaultValue,
		onValueChange,
		value,
	}: {
		children: React.ReactNode;
		defaultValue?: string;
		onValueChange?: (value: string) => void;
		value?: string;
	}) => (
		<TabsContext.Provider
			value={{ onValueChange, value: value ?? defaultValue ?? "" }}
		>
			<div>{children}</div>
		</TabsContext.Provider>
	),
	TabsList: ({ children }: { children: React.ReactNode }) => (
		<div>{children}</div>
	),
	TabsTrigger: ({
		children,
		value,
	}: {
		children: React.ReactNode;
		value: string;
	}) => {
		const context = useContext(TabsContext);
		return (
			<button
				type="button"
				data-value={value}
				onClick={() => context.onValueChange?.(value)}
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
		value: string;
	}) => {
		const context = useContext(TabsContext);
		return context.value === value ? <div>{children}</div> : null;
	},
}));

vi.mock("@/lib/preferenceSync", () => ({
	queuePreferenceSync: (...args: unknown[]) =>
		mockState.preferenceSync(...args),
}));

vi.mock("@/hooks/useApiError", () => ({
	handleApiError: vi.fn(),
}));

vi.mock("@/services/authService", () => ({
	authService: {
		updateProfile: (...args: unknown[]) =>
			mockState.authService.updateProfile(...args),
		setAvatarSource: (...args: unknown[]) =>
			mockState.authService.setAvatarSource(...args),
		uploadAvatar: (...args: unknown[]) =>
			mockState.authService.uploadAvatar(...args),
	},
}));

vi.mock("@/stores/authStore", () => ({
	useAuthStore: (selector: (state: typeof mockState.authStore) => unknown) =>
		selector(mockState.authStore),
}));

vi.mock("@/stores/fileStore", () => ({
	useFileStore: (selector: (state: typeof mockState.fileStore) => unknown) =>
		selector(mockState.fileStore),
}));

vi.mock("@/stores/themeStore", () => ({
	useThemeStore: () => mockState.themeStore,
}));

describe("SettingsPage", () => {
	beforeEach(() => {
		mockState.authService.setAvatarSource.mockReset();
		mockState.authService.uploadAvatar.mockReset();
		mockState.authService.updateProfile.mockReset();
		mockState.authStore.refreshUser.mockReset();
		mockState.changeLanguage.mockReset();
		mockState.fileStore.setViewMode.mockReset();
		mockState.fileStore.viewMode = "list";
		mockState.navigate.mockReset();
		mockState.preferenceSync.mockReset();
		mockState.themeStore.mode = "dark";
		mockState.themeStore.setMode.mockReset();
		mockState.translationLanguage = "zh-CN";
	});

	it("renders current descriptions from the selected theme, language, and browser mode", () => {
		render(<SettingsPage section="interface" />);

		expect(screen.getByTestId("app-layout")).toBeInTheDocument();
		expect(screen.getByText("settings")).toBeInTheDocument();
		expect(screen.getByText("settings:settings_page_desc")).toBeInTheDocument();
		expect(
			screen.getByText("settings:settings_theme_dark_desc"),
		).toBeInTheDocument();
		expect(
			screen.getByText("settings:settings_language_zh_desc"),
		).toBeInTheDocument();
		expect(
			screen.getByText("settings:settings_browser_list_desc"),
		).toBeInTheDocument();
		expect(screen.getByText("color-preset-picker")).toBeInTheDocument();
		expect(screen.getAllByTestId("choice-group")[0]).toHaveAttribute(
			"data-value",
			"dark",
		);
		expect(screen.getAllByTestId("choice-group")[1]).toHaveAttribute(
			"data-value",
			"zh",
		);
		expect(screen.getAllByTestId("choice-group")[2]).toHaveAttribute(
			"data-value",
			"list",
		);
	});

	it("dispatches theme, language, and browser preference changes", () => {
		render(<SettingsPage section="interface" />);

		fireEvent.click(screen.getByRole("button", { name: "theme_light" }));
		fireEvent.click(screen.getByRole("button", { name: "language_en" }));
		fireEvent.click(screen.getByRole("button", { name: "files:grid_view" }));

		expect(mockState.themeStore.setMode).toHaveBeenCalledWith("light");
		expect(mockState.changeLanguage).toHaveBeenCalledWith("en");
		expect(mockState.preferenceSync).toHaveBeenCalledWith({ language: "en" });
		expect(mockState.fileStore.setViewMode).toHaveBeenCalledWith("grid");
	});

	it("navigates between split settings sections from the top tabs", () => {
		render(<SettingsPage section="profile" />);

		fireEvent.click(
			screen.getByRole("button", { name: "settings:settings_interface" }),
		);

		expect(mockState.navigate).toHaveBeenCalledWith("/settings/interface", {
			viewTransition: true,
		});
	});

	it("saves the display name through the profile endpoint", async () => {
		render(<SettingsPage section="profile" />);

		expect(screen.getByText("avatar:alice")).toBeInTheDocument();
		expect(screen.getByDisplayValue("alice@example.com")).toBeInTheDocument();

		fireEvent.change(screen.getByLabelText("settings:settings_display_name"), {
			target: { value: "Alice Chen" },
		});
		fireEvent.click(screen.getByRole("button", { name: "save" }));

		await waitFor(() =>
			expect(mockState.authService.updateProfile).toHaveBeenCalledWith({
				display_name: "Alice Chen",
			}),
		);
		await waitFor(() =>
			expect(mockState.authStore.refreshUser).toHaveBeenCalledTimes(1),
		);
	});
});
