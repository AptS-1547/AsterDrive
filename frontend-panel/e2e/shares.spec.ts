import { authenticate, captureClientState } from "./support/auth";
import {
	createFolderFromSurface,
	createPageShare,
	dialogByTitle,
	fileNameCell,
	navigateToRoot,
	openFolder,
	uploadViaPicker,
} from "./support/files";
import { IMAGE_FILE, uniqueName } from "./support/fixtures";
import { apiJsonInPage } from "./support/network";
import {
	expectProtectedFolderSharePreview,
	expectShareCardMissing,
	expectShareCardVisible,
	expectShareUnavailable,
	extractShareToken,
	gotoMyShares,
	shareSelectionToggle,
} from "./support/shares";
import { expect, test } from "./support/test";

test.describe
	.serial("Share E2E", () => {
		test("opens a password-protected folder share and previews files anonymously", async ({
			browser,
			page,
			request,
		}) => {
			await authenticate(page, request);

			const sharedFolder = "pw-share-protected-folder";
			const sharedImage = {
				...IMAGE_FILE,
				name: "pw-share-protected-image.gif",
			} as const;
			const sharePassword = "PlaywrightShare123!";

			await createFolderFromSurface(page, sharedFolder);
			await openFolder(page, sharedFolder);
			await uploadViaPicker(page, [sharedImage]);
			await expect(fileNameCell(page, sharedImage.name)).toBeVisible({
				timeout: 30_000,
			});

			await navigateToRoot(page);
			const shareUrl = await createPageShare(page, sharedFolder, {
				password: sharePassword,
			});
			const clientState = await captureClientState(page);

			await expectProtectedFolderSharePreview(
				browser,
				shareUrl,
				sharedImage.name,
				sharePassword,
				clientState,
			);
		});

		test("manages shares from My Shares and revokes deleted links", async ({
			browser,
			page,
			request,
		}) => {
			await authenticate(page, request);

			const firstFile = {
				buffer: Buffer.from("share management alpha\n", "utf8"),
				mimeType: "text/plain",
				name: `${uniqueName("pw-share-manage-alpha")}.txt`,
			} as const;
			const secondFile = {
				buffer: Buffer.from("share management beta\n", "utf8"),
				mimeType: "text/plain",
				name: `${uniqueName("pw-share-manage-beta")}.txt`,
			} as const;

			await uploadViaPicker(page, [firstFile, secondFile]);
			await expect(fileNameCell(page, firstFile.name)).toBeVisible({
				timeout: 30_000,
			});
			await expect(fileNameCell(page, secondFile.name)).toBeVisible({
				timeout: 30_000,
			});

			const firstShareUrl = await createPageShare(page, firstFile.name);
			const secondShareUrl = await createPageShare(page, secondFile.name);

			await gotoMyShares(page);
			await expectShareCardVisible(page, firstFile.name);
			await expectShareCardVisible(page, secondFile.name);

			await shareSelectionToggle(page, firstFile.name).click();
			await shareSelectionToggle(page, secondFile.name).click();
			await expect(
				page.getByText("2 selected", { exact: true }).first(),
			).toBeVisible();

			await page
				.getByRole("button", { exact: true, name: "Remove selected" })
				.click();
			const deleteDialog = page.getByRole("alertdialog");
			await expect(deleteDialog).toBeVisible();
			await deleteDialog
				.getByRole("button", { exact: true, name: "Delete" })
				.click();

			await expectShareCardMissing(page, firstFile.name);
			await expectShareCardMissing(page, secondFile.name);

			await expectShareUnavailable(browser, firstShareUrl);
			await expectShareUnavailable(browser, secondShareUrl);
		});

		test("updates share settings from My Shares", async ({ page, request }) => {
			await authenticate(page, request);

			const file = {
				buffer: Buffer.from("share edit\n", "utf8"),
				mimeType: "text/plain",
				name: `${uniqueName("pw-share-edit")}.txt`,
			} as const;

			await uploadViaPicker(page, [file]);
			await expect(fileNameCell(page, file.name)).toBeVisible({
				timeout: 30_000,
			});

			const shareUrl = await createPageShare(page, file.name);
			const shareToken = extractShareToken(shareUrl);

			await gotoMyShares(page);
			await expectShareCardVisible(page, file.name);

			await shareSelectionToggle(page, file.name).click();
			await expect(
				page.getByText("1 selected", { exact: true }).first(),
			).toBeVisible();
			await page.getByRole("button", { exact: true, name: "Edit" }).click();

			const editDialog = dialogByTitle(page, `Edit share "${file.name}"`);
			await expect(editDialog).toBeVisible();
			await editDialog.locator("#edit-share-max-downloads").fill("1");
			await editDialog
				.getByRole("button", { exact: true, name: "Save" })
				.click();
			await expect(editDialog).toBeHidden();

			const sharePage = await apiJsonInPage<{
				items: Array<{
					max_downloads: number;
					resource_name: string;
					token: string;
				}>;
				total: number;
			}>(page, "/api/v1/shares?limit=50&offset=0");
			const updatedShare = sharePage.items.find(
				(item) => item.token === shareToken,
			);

			expect(updatedShare).toBeTruthy();
			expect(updatedShare?.resource_name).toBe(file.name);
			expect(updatedShare?.max_downloads).toBe(1);
		});
	});
