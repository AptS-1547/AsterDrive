import { type Browser, expect, type Page } from "@playwright/test";
import { seedClientState } from "./auth";
import { chooseOpenMethodIfPrompted, closeActiveDialog } from "./files";

export async function expectAnonymousSharePreview(
	browser: Browser,
	shareUrl: string,
	fileName: string,
	clientState: Record<string, string>,
) {
	const context = await browser.newContext({
		acceptDownloads: true,
		locale: "en-US",
		serviceWorkers: "block",
	});
	const page = await context.newPage();

	await seedClientState(page, clientState);
	await page.goto(shareUrl);

	await expect(page.getByText(fileName, { exact: true })).toBeVisible();
	await page.getByRole("button", { name: "Preview" }).click();
	await chooseOpenMethodIfPrompted(page, "Image preview", 5_000);

	const dialog = page.getByRole("dialog");
	await expect(dialog.getByRole("img", { name: fileName })).toBeVisible({
		timeout: 30_000,
	});

	await closeActiveDialog(page);
	await context.close();
}

export async function expectProtectedFolderSharePreview(
	browser: Browser,
	shareUrl: string,
	fileName: string,
	password: string,
	clientState: Record<string, string>,
) {
	const context = await browser.newContext({
		acceptDownloads: true,
		locale: "en-US",
		serviceWorkers: "block",
	});
	const page = await context.newPage();

	await seedClientState(page, clientState);
	await page.goto(shareUrl);

	await expect(
		page.getByText("This share is password protected"),
	).toBeVisible();
	await page.getByPlaceholder("Password").fill(password);
	await page.getByRole("button", { name: "Verify" }).click();

	await expect(page.getByText(fileName, { exact: true })).toBeVisible({
		timeout: 30_000,
	});
	await page
		.getByRole("button")
		.filter({ has: page.getByText(fileName, { exact: true }) })
		.first()
		.click();
	await chooseOpenMethodIfPrompted(page, "Image preview", 5_000);

	const dialog = page.getByRole("dialog");
	await expect(dialog.getByRole("img", { name: fileName })).toBeVisible({
		timeout: 30_000,
	});

	await closeActiveDialog(page);
	await context.close();
}

export async function expectShareUnavailable(
	browser: Browser,
	shareUrl: string,
	expectedError = "This share link does not exist or is no longer available",
) {
	const context = await browser.newContext({
		locale: "en-US",
		serviceWorkers: "block",
	});
	const page = await context.newPage();

	await seedClientState(page);
	await page.goto(shareUrl);
	await expect(page).toHaveURL(shareUrl);
	await expect(page.getByText("Unavailable", { exact: true })).toBeVisible({
		timeout: 30_000,
	});
	await expect(page.getByText(expectedError, { exact: true })).toBeVisible({
		timeout: 30_000,
	});

	await context.close();
}

export async function gotoMyShares(page: Page) {
	await page.goto("/shares");
	await expect(
		page.getByRole("heading", { exact: true, name: "My Shares" }),
	).toBeVisible({
		timeout: 30_000,
	});
}

export function shareCard(page: Page, resourceName: string) {
	return page
		.locator('div[role="button"]')
		.filter({ has: page.getByText(resourceName, { exact: true }) })
		.first();
}

export function shareSelectionToggle(page: Page, resourceName: string) {
	return shareCard(page, resourceName).locator("button[aria-pressed]").first();
}

export async function expectShareCardVisible(page: Page, resourceName: string) {
	await expect(shareCard(page, resourceName)).toBeVisible({ timeout: 30_000 });
}

export async function expectShareCardMissing(page: Page, resourceName: string) {
	await expect(shareCard(page, resourceName)).toHaveCount(0, {
		timeout: 30_000,
	});
}

export function extractShareToken(shareUrl: string) {
	const match = shareUrl.match(/\/s\/([^/?#]+)/);
	if (!match) {
		throw new Error(`Could not extract share token from URL: ${shareUrl}`);
	}

	return match[1];
}
