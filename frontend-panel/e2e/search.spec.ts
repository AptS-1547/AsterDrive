import type { Locator } from "@playwright/test";
import { authenticate } from "./support/auth";
import {
	createFolderFromSurface,
	fileNameCell,
	navigateToRoot,
	openFolder,
	uploadViaPicker,
} from "./support/files";
import { uniqueName } from "./support/fixtures";
import { expect, test } from "./support/test";

async function openSearchFilters(searchDialog: Locator, targetName: string) {
	const target = searchDialog.getByRole("button", { name: targetName });
	const alreadyVisible = await target
		.waitFor({ state: "visible", timeout: 1_000 })
		.then(() => true)
		.catch(() => false);
	if (!alreadyVisible) {
		await searchDialog.getByRole("button", { name: "Filters" }).click();
	}
	await expect(target).toBeVisible();
}

test.describe
	.serial("Search E2E", () => {
		test("searches files and folders in the current workspace", async ({
			page,
			request,
		}) => {
			await authenticate(page, request);

			const token = uniqueName("pw-search");
			const folderName = `${token}-folder`;
			const file = {
				buffer: Buffer.from("Searchable content from Playwright\n", "utf8"),
				mimeType: "text/plain",
				name: `${token}-note.txt`,
			} as const;

			await uploadViaPicker(page, [file]);
			await expect(fileNameCell(page, file.name)).toBeVisible({
				timeout: 30_000,
			});

			await createFolderFromSurface(page, folderName);

			await page.getByRole("button", { name: "Open search" }).first().click();
			const searchDialog = page.getByRole("dialog");
			await expect(searchDialog).toBeVisible();
			await searchDialog
				.getByPlaceholder("Search files and folders...")
				.fill(token);
			await searchDialog
				.getByRole("button", { exact: true, name: "Search" })
				.click();
			await expect(searchDialog).toBeHidden();
			await expect(page).toHaveURL(/\/search\?q=.*&type=all/);
			await expect(fileNameCell(page, file.name)).toBeVisible({
				timeout: 30_000,
			});
			await expect(fileNameCell(page, folderName)).toBeVisible({
				timeout: 30_000,
			});

			await page.getByRole("button", { name: "Open search" }).first().click();
			await expect(searchDialog).toBeVisible();
			await searchDialog
				.getByPlaceholder("Search files and folders...")
				.fill(token);
			await openSearchFilters(searchDialog, "Files only");
			await searchDialog.getByRole("button", { name: "Files only" }).click();
			await searchDialog
				.getByRole("button", { exact: true, name: "Search" })
				.click();
			await expect(page).toHaveURL(/\/search\?q=.*&type=file/);
			await expect(fileNameCell(page, file.name)).toBeVisible({
				timeout: 30_000,
			});
			await expect(fileNameCell(page, folderName)).toHaveCount(0);

			await page.getByRole("button", { name: "Open search" }).first().click();
			await expect(searchDialog).toBeVisible();
			await searchDialog
				.getByPlaceholder("Search files and folders...")
				.fill(token);
			await openSearchFilters(searchDialog, "Folders only");
			await searchDialog.getByRole("button", { name: "Folders only" }).click();
			await searchDialog
				.getByRole("button", { exact: true, name: "Search" })
				.click();
			await expect(page).toHaveURL(/\/search\?q=.*&type=folder/);
			await expect(fileNameCell(page, folderName)).toBeVisible({
				timeout: 30_000,
			});
			await expect(fileNameCell(page, file.name)).toHaveCount(0);

			await openFolder(page, folderName);
			await expect(page).toHaveURL(/\/folder\/\d+/);
			await expect(
				page
					.getByRole("navigation", { name: "Breadcrumb" })
					.getByText(folderName, { exact: true }),
			).toBeVisible();

			await navigateToRoot(page);
			await openFolder(page, folderName);
		});
	});
