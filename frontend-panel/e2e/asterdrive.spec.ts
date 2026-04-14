import { readFile } from "node:fs/promises";
import path from "node:path";
import {
	type APIRequestContext,
	type Browser,
	expect,
	type Page,
	test,
} from "@playwright/test";

const ADMIN = {
	email: "admin@example.com",
	password: "Playwright123!",
	username: "admin",
} as const;

const CODE_CONTENT = `const greeting = "Hello from Playwright";
console.log(greeting);
`;

const IMAGE_FILE = {
	buffer: Buffer.from(
		"R0lGODdhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==",
		"base64",
	),
	mimeType: "image/gif",
	name: "playwright-image.gif",
} as const;

const PDF_FILE = {
	buffer: createMinimalPdf("Playwright PDF"),
	mimeType: "application/pdf",
	name: "playwright-document.pdf",
} as const;

const CODE_FILE = {
	buffer: Buffer.from(CODE_CONTENT, "utf8"),
	mimeType: "text/typescript",
	name: "playwright-code.ts",
} as const;

const PREVIEW_APPS_CACHE_KEY = "aster-cached-preview-apps";
const RESUMABLE_UPLOADS_KEY = "aster_resumable_uploads";
const RUN_ID = Date.now().toString(36);

const DEFAULT_STORAGE_STATE = {
	"aster-browser-open-mode": "single_click",
	"aster-language": "en",
	"aster-sort-by": "name",
	"aster-sort-order": "asc",
	"aster-view-mode": "list",
} as const;

test.describe
	.serial("AsterDrive E2E", () => {
		test.beforeEach(async ({ page }) => {
			await seedClientState(page);
		});

		test("creates the initial admin, logs out, and signs back in", async ({
			page,
			request,
		}) => {
			expect(await hasUsers(request)).toBe(false);
			await setupAdmin(page);
			expect(await hasUsers(request)).toBe(true);

			await openUserMenu(page);
			await page.getByRole("button", { name: "Logout" }).click();
			await expect(page).toHaveURL(/\/login$/);

			await loginAsAdmin(page);
		});

		test("uploads, previews, downloads, and opens an anonymous share page", async ({
			browser,
			page,
			request,
		}, testInfo) => {
			await authenticate(page, request);

			await uploadViaPicker(page, [IMAGE_FILE, PDF_FILE]);
			await uploadViaDragDrop(page, [CODE_FILE]);

			for (const fileName of [IMAGE_FILE.name, PDF_FILE.name, CODE_FILE.name]) {
				await expect(fileNameCell(page, fileName)).toBeVisible({
					timeout: 30_000,
				});
			}

			await expectImagePreview(page, IMAGE_FILE.name);
			await closeActiveDialog(page);

			await expectPdfPreview(page, PDF_FILE.name);
			await closeActiveDialog(page);

			await expectCodePreview(page, CODE_FILE.name);
			await closeActiveDialog(page);

			await expectDownloadMatches(
				page,
				CODE_FILE.name,
				CODE_FILE.buffer,
				testInfo.outputDir,
			);

			const shareUrl = await createPageShare(page, IMAGE_FILE.name);
			const clientState = await captureClientState(page);
			await expectAnonymousSharePreview(
				browser,
				shareUrl,
				IMAGE_FILE.name,
				clientState,
			);
		});

		test("manages folders, files, and trash lifecycle flows", async ({
			page,
			request,
		}) => {
			await authenticate(page, request);

			const projectFolder = "pw-life-projects";
			const archiveFolder = "pw-life-archive";
			const referencesFolder = "pw-life-references";
			const lifecycleFile = {
				buffer: Buffer.from("Lifecycle flow from Playwright\n", "utf8"),
				mimeType: "text/plain",
				name: "pw-life-note.txt",
			} as const;
			const renamedLifecycleFile = "pw-life-note-renamed.txt";

			await createFolderFromSurface(page, projectFolder);
			await createFolderFromSurface(page, archiveFolder);

			await uploadViaPicker(page, [lifecycleFile]);
			await expect(fileNameCell(page, lifecycleFile.name)).toBeVisible({
				timeout: 30_000,
			});

			await renameItem(page, lifecycleFile.name, renamedLifecycleFile);
			await expect(fileNameCell(page, renamedLifecycleFile)).toBeVisible({
				timeout: 30_000,
			});

			await copyItemToFolder(page, renamedLifecycleFile, archiveFolder);
			await openFolder(page, archiveFolder);
			await expect(fileNameCell(page, renamedLifecycleFile)).toBeVisible({
				timeout: 30_000,
			});

			await navigateToRoot(page);
			await renameItem(page, archiveFolder, referencesFolder);
			await expect(fileNameCell(page, referencesFolder)).toBeVisible({
				timeout: 30_000,
			});

			await moveItemToFolder(page, renamedLifecycleFile, projectFolder);
			await expectItemMissing(page, renamedLifecycleFile);

			await openFolder(page, projectFolder);
			await expect(fileNameCell(page, renamedLifecycleFile)).toBeVisible({
				timeout: 30_000,
			});

			await navigateToRoot(page);
			await deleteItem(page, projectFolder);
			await deleteItem(page, referencesFolder);
			await expectItemMissing(page, projectFolder);
			await expectItemMissing(page, referencesFolder);

			await page.getByRole("link", { name: "Trash" }).click();
			await expect(page).toHaveURL(/\/trash$/);
			await expectTrashItemVisible(page, projectFolder);
			await expectTrashItemVisible(page, referencesFolder);

			await trashItemRow(page, projectFolder).click();
			await page.getByRole("button", { name: "Restore Selected" }).click();
			await expectTrashItemMissing(page, projectFolder);

			await trashItemRow(page, referencesFolder).click();
			await page
				.getByRole("button", { name: "Delete Selected Permanently" })
				.click();
			await page.getByRole("button", { name: "Permanently Delete" }).click();
			await expectTrashItemMissing(page, referencesFolder);

			await page.getByRole("link", { name: "My Drive" }).click();
			await expect(fileDropZone(page)).toBeVisible();
			await expect(folderTreeButton(page, projectFolder)).toBeVisible({
				timeout: 30_000,
			});
			await expect(folderTreeButton(page, referencesFolder)).toHaveCount(0);

			await openFolder(page, projectFolder);
			await expect(fileNameCell(page, renamedLifecycleFile)).toBeVisible({
				timeout: 30_000,
			});
		});

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

		test("manages admin users end-to-end", async ({ page, request }) => {
			await authenticate(page, request);

			const username = uniqueAccountName("pwuser");
			const email = `${username}@example.com`;

			await gotoAdminPage(page, "/admin/users", "Users");

			await page.getByRole("button", { name: "New User" }).click();
			const createDialog = dialogByTitle(page, "Create user");
			await expect(createDialog).toBeVisible();
			await createDialog.locator("#create-user-username").fill(username);
			await createDialog.locator("#create-user-email").fill(email);
			await createDialog.locator("#create-user-password").fill("Playwright123!");
			await createDialog.getByRole("button", { name: "Create" }).click();
			await expect(createDialog).toBeHidden();

			await expect(tableRowByCellText(page, username)).toBeVisible({
				timeout: 30_000,
			});

			await page
				.getByPlaceholder("Search by username, email, or ID...")
				.fill(username);
			await expect(tableRowByCellText(page, username)).toBeVisible({
				timeout: 30_000,
			});

			await tableRowByCellText(page, username).click();
			const detailDialog = dialogByTitle(page, "User details");
			await expect(detailDialog).toBeVisible();
			await expect(detailDialog.getByText(email, { exact: true })).toBeVisible();
			await detailDialog
				.locator('[data-slot="dialog-footer"]')
				.getByRole("button", { name: "Close" })
				.click();
			await expect(detailDialog).toBeHidden();

			await tableRowByCellText(page, username)
				.getByTitle("Delete user")
				.click();
			const deleteDialog = page.getByRole("alertdialog", {
				name: "Delete user",
			});
			await expect(deleteDialog).toBeVisible();
			await deleteDialog.getByRole("button", { name: "Delete" }).click();
			await expect(tableRowByCellText(page, username)).toHaveCount(0, {
				timeout: 30_000,
			});
		});

		test("configures local storage policies through the admin flow", async ({
			page,
			request,
		}) => {
			await authenticate(page, request);

			const policyName = uniqueName("pw-local-policy");
			const initialBasePath = `/tmp/${policyName}-v1`;
			const updatedBasePath = `/tmp/${policyName}-v2`;

			await gotoAdminPage(page, "/admin/policies", "Storage Policies");

			await page.getByRole("button", { name: "New Policy" }).click();
			const createDialog = dialogByTitle(page, "Create Policy");
			await expect(createDialog).toBeVisible();
			await createDialog.getByRole("button", { name: "Local" }).click();
			await createDialog.getByRole("button", { name: "Next" }).click();
			await createDialog.locator("#name").fill(policyName);
			await createDialog.locator("#base_path").fill(initialBasePath);
			await createDialog
				.getByRole("button", { exact: true, name: "Review" })
				.click();
			await createDialog.getByRole("button", { name: "Create" }).click();
			await expect(createDialog).toBeHidden();

			await expect(tableRowByCellText(page, policyName)).toBeVisible({
				timeout: 30_000,
			});
			await expect(tableRowByCellText(page, policyName)).toContainText(
				initialBasePath,
			);

			await tableRowByCellText(page, policyName).click();
			const editDialog = dialogByTitle(page, "Edit Policy");
			await expect(editDialog).toBeVisible();
			await editDialog.locator("#base_path").fill(updatedBasePath);
			await editDialog.getByRole("button", { name: "Save Changes" }).click();
			await expect(editDialog).toBeHidden();
			await expect(tableRowByCellText(page, policyName)).toContainText(
				updatedBasePath,
			);

			await tableRowByCellText(page, policyName)
				.getByTitle("Delete Policy")
				.click();
			const deleteDialog = page.getByRole("alertdialog", {
				name: `Delete Policy "${policyName}"?`,
			});
			await expect(deleteDialog).toBeVisible();
			await deleteDialog.getByRole("button", { name: "Delete" }).click();
			await expect(tableRowByCellText(page, policyName)).toHaveCount(0, {
				timeout: 30_000,
			});
		});

		test("applies batch copy, move, and delete operations from multi-selection", async ({
			page,
			request,
		}) => {
			await authenticate(page, request);

			const copyTarget = uniqueName("pw-batch-copy");
			const moveTarget = uniqueName("pw-batch-move");
			const firstFile = {
				buffer: Buffer.from("batch file alpha\n", "utf8"),
				mimeType: "text/plain",
				name: `${uniqueName("pw-batch-alpha")}.txt`,
			} as const;
			const secondFile = {
				buffer: Buffer.from("batch file beta\n", "utf8"),
				mimeType: "text/plain",
				name: `${uniqueName("pw-batch-beta")}.txt`,
			} as const;

			await createFolderFromSurface(page, copyTarget);
			await createFolderFromSurface(page, moveTarget);
			await uploadViaPicker(page, [firstFile, secondFile]);
			await expect(fileNameCell(page, firstFile.name)).toBeVisible({
				timeout: 30_000,
			});
			await expect(fileNameCell(page, secondFile.name)).toBeVisible({
				timeout: 30_000,
			});

			await toggleItemSelection(page, firstFile.name);
			await toggleItemSelection(page, secondFile.name);
			await expect(page.getByText("2 selected")).toBeVisible();
			await page.getByRole("button", { exact: true, name: "Copy" }).click();
			await chooseTargetFolder(page, copyTarget, "Copy here");

			await openFolder(page, copyTarget);
			await expect(fileNameCell(page, firstFile.name)).toBeVisible({
				timeout: 30_000,
			});
			await expect(fileNameCell(page, secondFile.name)).toBeVisible({
				timeout: 30_000,
			});

			await navigateToRoot(page);
			await toggleItemSelection(page, firstFile.name);
			await toggleItemSelection(page, secondFile.name);
			await page.getByRole("button", { exact: true, name: "Move" }).click();
			await chooseTargetFolder(page, moveTarget, "Move here");
			await expectItemMissing(page, firstFile.name);
			await expectItemMissing(page, secondFile.name);

			await openFolder(page, moveTarget);
			await expect(fileNameCell(page, firstFile.name)).toBeVisible({
				timeout: 30_000,
			});
			await expect(fileNameCell(page, secondFile.name)).toBeVisible({
				timeout: 30_000,
			});

			await toggleItemSelection(page, firstFile.name);
			await toggleItemSelection(page, secondFile.name);
			await page.getByRole("button", { exact: true, name: "Delete" }).click();
			const deleteDialog = page.getByRole("alertdialog");
			await expect(deleteDialog).toBeVisible();
			await deleteDialog.getByRole("button", { name: "Delete" }).click();
			await expectItemMissing(page, firstFile.name);
			await expectItemMissing(page, secondFile.name);

			await page.getByRole("link", { name: "Trash" }).click();
			await expect(page).toHaveURL(/\/trash$/);
			await expectTrashItemVisible(page, firstFile.name);
			await expectTrashItemVisible(page, secondFile.name);
		});

		test("resumes a chunked upload from persisted progress", async ({
			page,
			request,
		}) => {
			await authenticate(page, request);

			const filename = `${uniqueName("pw-resume")}.bin`;
			const buffer = Buffer.alloc(6 * 1024 * 1024 + 257, 0x61);
			const init = await apiJsonInPage<{
				chunk_size: number;
				mode?: string;
				total_chunks: number;
				upload_id: string;
			}>(page, "/api/v1/files/upload/init", {
				method: "POST",
				body: {
					filename,
					total_size: buffer.length,
				},
				withCsrf: true,
			});
			expect(init.total_chunks).toBeGreaterThan(1);
			expect(init.chunk_size).toBeGreaterThan(0);

			await uploadChunkViaApi(
				page,
				init.upload_id,
				0,
				buffer.subarray(0, init.chunk_size),
			);

			const progress = await apiJsonInPage<{
				received_count: number;
				status: string;
			}>(page, `/api/v1/files/upload/${init.upload_id}`);
			expect(progress.received_count).toBe(1);
			expect(progress.status).toBe("uploading");

			await saveResumableSession(page, {
				baseFolderId: null,
				baseFolderName: "My Drive",
				chunkSize: init.chunk_size,
				filename,
				mode: "chunked",
				relativePath: null,
				savedAt: Date.now(),
				totalChunks: init.total_chunks,
				totalSize: buffer.length,
				uploadId: init.upload_id,
				workspace: { kind: "personal" },
			});

			await page.reload();
			await expect(page.getByText(filename, { exact: true })).toBeVisible({
				timeout: 30_000,
			});
			await expect(page.getByText("Chunked", { exact: true })).toBeVisible();
			await expect(
				page.getByText(`Chunk 1/${init.total_chunks}`, { exact: true }),
			).toBeVisible();
			await page.getByTitle("Select file to resume").first().click();
			await page.getByTestId("resume-input").setInputFiles({
				buffer,
				mimeType: "application/octet-stream",
				name: filename,
			});

			await expect(fileNameCell(page, filename)).toBeVisible({
				timeout: 30_000,
			});
			expect(await loadPersistedSessions(page)).toHaveLength(0);
		});

		test("creates a WebDAV account and exercises basic WebDAV methods", async ({
			page,
			request,
		}) => {
			await authenticate(page, request);

			const username = uniqueAccountName("pwdav");
			const password = "PlaywrightDav123!";
			const settings = await apiJsonInPage<{
				endpoint: string;
				prefix: string;
			}>(page, "/api/v1/webdav-accounts/settings");
			const prefix = normalizeWebdavPrefix(settings.prefix);
			const authHeader = basicAuth(username, password);
			const directoryName = uniqueName("pw-dav-dir");
			const fileName = `${uniqueName("pw-dav-file")}.txt`;
			const fileContent = "WebDAV content from Playwright";
			const rootPath = `${prefix}/`;
			const directoryPath = `${rootPath}${directoryName}/`;
			const filePath = `${directoryPath}${fileName}`;

			await page.goto("/settings/webdav");
			await expect(
				page.getByRole("heading", { exact: true, name: "WebDAV" }),
			).toBeVisible();
			await page.getByRole("button", { name: "Create WebDAV Account" }).click();

			const createDialog = dialogByTitle(page, "Create WebDAV Account");
			await expect(createDialog).toBeVisible();
			await createDialog.locator("#username").fill(username);
			await createDialog.locator("#password").fill(password);
			await createDialog.getByRole("button", { name: "Create" }).click();

			const credentialsDialog = dialogByTitle(page, "Latest Credentials");
			await expect(credentialsDialog).toBeVisible();
			await credentialsDialog.getByRole("button", { name: "Test Connection" }).click();
			await expect(
				credentialsDialog.getByText("Connection successful", {
					exact: true,
				}),
			).toBeVisible({
				timeout: 30_000,
			});

			const rootListing = await webdavRequest(page, rootPath, {
				headers: {
					Authorization: authHeader,
					Depth: "0",
				},
				method: "PROPFIND",
			});
			expect(rootListing.status).toBe(207);

			const createDirectory = await webdavRequest(page, directoryPath, {
				headers: {
					Authorization: authHeader,
				},
				method: "MKCOL",
			});
			expect(createDirectory.status).toBe(201);

			const nestedListing = await webdavRequest(page, rootPath, {
				headers: {
					Authorization: authHeader,
					Depth: "1",
				},
				method: "PROPFIND",
			});
			expect(nestedListing.status).toBe(207);
			expect(nestedListing.text).toContain(directoryName);

			const putFile = await webdavRequest(page, filePath, {
				body: fileContent,
				headers: {
					Authorization: authHeader,
					"Content-Type": "text/plain",
				},
				method: "PUT",
			});
			expect([201, 204]).toContain(putFile.status);

			const getFile = await webdavRequest(page, filePath, {
				headers: {
					Authorization: authHeader,
				},
				method: "GET",
			});
			expect(getFile.status).toBe(200);
			expect(getFile.text).toContain(fileContent);

			const deleteFile = await webdavRequest(page, filePath, {
				headers: {
					Authorization: authHeader,
				},
				method: "DELETE",
			});
			expect([200, 204]).toContain(deleteFile.status);

			const deleteDirectory = await webdavRequest(page, directoryPath, {
				headers: {
					Authorization: authHeader,
				},
				method: "DELETE",
			});
			expect([200, 204]).toContain(deleteDirectory.status);
		});

		test("supports mobile navigation for the user and admin layouts", async ({
			page,
			request,
		}) => {
			await authenticate(page, request);

			await page.setViewportSize({ width: 390, height: 844 });
			await page.goto("/");
			await expect(fileDropZone(page)).toBeVisible();

			await page.getByRole("button", { name: "Open sidebar" }).click();
			await expect(
				page.getByRole("button", { name: "Close sidebar" }).first(),
			).toBeVisible();
			await page.getByRole("link", { name: "Trash" }).click();
			await expect(page).toHaveURL(/\/trash$/);

			await gotoAdminPage(page, "/admin/users", "Users");
			await page.getByRole("button", { name: "Open admin sidebar" }).click();
			await page.getByRole("link", { name: "Storage Policies" }).click();
			await expect(page).toHaveURL(/\/admin\/policies$/);
		});
	});

async function seedClientState(
	page: Page,
	entries: Record<string, string> = DEFAULT_STORAGE_STATE,
) {
	await page.addInitScript((entries) => {
		for (const [key, value] of Object.entries(entries)) {
			window.localStorage.setItem(key, value);
		}
	}, entries);
}

async function captureClientState(page: Page) {
	const entries = { ...DEFAULT_STORAGE_STATE };
	const cachedPreviewApps = await page.evaluate(
		(key) => window.localStorage.getItem(key),
		PREVIEW_APPS_CACHE_KEY,
	);

	if (cachedPreviewApps) {
		entries[PREVIEW_APPS_CACHE_KEY] = cachedPreviewApps;
	}

	return entries;
}

function createMinimalPdf(text: string) {
	const escapedText = text
		.replaceAll("\\", "\\\\")
		.replaceAll("(", "\\(")
		.replaceAll(")", "\\)");
	const contentStream = `BT
/F1 18 Tf
48 96 Td
(${escapedText}) Tj
ET`;
	const objects = [
		"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
		"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n",
		"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 144] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n",
		`4 0 obj\n<< /Length ${Buffer.byteLength(contentStream, "utf8")} >>\nstream\n${contentStream}\nendstream\nendobj\n`,
		"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
	];
	let pdf = "%PDF-1.4\n";
	const offsets = [0];

	for (const object of objects) {
		offsets.push(Buffer.byteLength(pdf, "utf8"));
		pdf += object;
	}

	const xrefOffset = Buffer.byteLength(pdf, "utf8");
	pdf += `xref
0 ${objects.length + 1}
0000000000 65535 f 
`;

	for (let index = 1; index <= objects.length; index += 1) {
		pdf += `${String(offsets[index]).padStart(10, "0")} 00000 n 
`;
	}

	pdf += `trailer
<< /Size ${objects.length + 1} /Root 1 0 R >>
startxref
${xrefOffset}
%%EOF
`;

	return Buffer.from(pdf, "utf8");
}

function fileDropZone(page: Page) {
	return page.locator('[aria-label="File drop zone"]').first();
}

function fileNameCell(page: Page, fileName: string) {
	return page.getByRole("cell", { exact: true, name: fileName }).first();
}

function tableRowByCellText(page: Page, cellText: string) {
	return page
		.getByRole("row")
		.filter({ has: page.getByRole("cell", { exact: true, name: cellText }) })
		.first();
}

function fileRow(page: Page, fileName: string) {
	return tableRowByCellText(page, fileName);
}

function dialogByTitle(page: Page, title: string) {
	return page
		.getByRole("dialog")
		.filter({ has: page.getByRole("heading", { exact: true, name: title }) })
		.first();
}

async function resolveAdminSiteUrlPrompt(page: Page) {
	const dialog = page.getByRole("alertdialog", {
		name: "Current site URL does not match the system config",
	});
	const promptAppeared = await dialog
		.waitFor({ state: "visible", timeout: 3_000 })
		.then(() => true)
		.catch(() => false);
	if (!promptAppeared) {
		return;
	}
	await dialog.getByRole("button", { name: "Update site URL" }).click();
	await expect(dialog).toBeHidden({ timeout: 30_000 });
}

async function gotoAdminPage(page: Page, url: string, heading: string) {
	await page.goto(url);
	await resolveAdminSiteUrlPrompt(page);
	await expect(page.getByRole("heading", { name: heading })).toBeVisible({
		timeout: 30_000,
	});
}

async function hasUsers(request: APIRequestContext) {
	const response = await request.post("/api/v1/auth/check");
	expect(response.ok()).toBe(true);
	const payload = (await response.json()) as {
		code: number;
		data?: {
			has_users?: boolean;
		} | null;
	};
	expect(payload.code).toBe(0);
	return payload.data?.has_users ?? false;
}

async function authenticate(page: Page, request: APIRequestContext) {
	if (await hasUsers(request)) {
		await loginAsAdmin(page);
		return;
	}

	await setupAdmin(page);
}

async function setupAdmin(page: Page) {
	await page.goto("/login");
	await expect(page.locator("#extra")).toBeVisible();
	await page.locator("#identifier").fill(ADMIN.email);
	await page.locator("#extra").fill(ADMIN.username);
	await page.locator("#password").fill(ADMIN.password);
	await page.locator("form button[type='submit']").click();
	await expect(page).toHaveURL(/\/$/);
	await expect(fileDropZone(page)).toBeVisible();
}

async function loginAsAdmin(page: Page) {
	await page.goto("/login");
	await page.locator("#identifier").fill(ADMIN.email);
	await page.locator("#password").fill(ADMIN.password);
	await page.locator("form button[type='submit']").click();
	await expect(page).toHaveURL(/\/$/);
	await expect(fileDropZone(page)).toBeVisible();
}

async function openUserMenu(page: Page) {
	await page.getByRole("button", { name: ADMIN.username }).click();
}

async function uploadViaPicker(
	page: Page,
	files: ReadonlyArray<{
		buffer: Buffer;
		mimeType: string;
		name: string;
	}>,
) {
	await page.getByTestId("upload-file-input").setInputFiles(
		files.map((file) => ({
			buffer: file.buffer,
			mimeType: file.mimeType,
			name: file.name,
		})),
	);
}

async function uploadViaDragDrop(
	page: Page,
	files: ReadonlyArray<{
		buffer: Buffer;
		mimeType: string;
		name: string;
	}>,
) {
	const dataTransfer = await page.evaluateHandle(
		(payload) => {
			const transfer = new DataTransfer();

			for (const file of payload) {
				const binary = atob(file.bufferBase64);
				const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
				transfer.items.add(
					new File([bytes], file.name, { type: file.mimeType }),
				);
			}

			return transfer;
		},
		files.map((file) => ({
			bufferBase64: file.buffer.toString("base64"),
			mimeType: file.mimeType,
			name: file.name,
		})),
	);

	const dropZone = fileDropZone(page);
	await dropZone.dispatchEvent("dragenter", { dataTransfer });
	await dropZone.dispatchEvent("dragover", { dataTransfer });
	await dropZone.dispatchEvent("drop", { dataTransfer });
}

async function openSurfaceContextMenu(page: Page) {
	const trigger = fileDropZone(page)
		.locator('[data-slot="context-menu-trigger"]')
		.first();
	await trigger.evaluate((element) => {
		const rect = element.getBoundingClientRect();
		element.dispatchEvent(
			new MouseEvent("contextmenu", {
				bubbles: true,
				button: 2,
				cancelable: true,
				clientX: rect.left + Math.max(24, rect.width - 80),
				clientY: rect.top + Math.max(24, rect.height - 80),
				view: window,
			}),
		);
	});
}

async function createFolderFromSurface(page: Page, folderName: string) {
	await openSurfaceContextMenu(page);
	await page.getByRole("menuitem", { name: "New Folder" }).click();

	const dialog = page.getByRole("dialog");
	await expect(dialog).toBeVisible();
	await dialog.getByPlaceholder("Folder name").fill(folderName);
	await dialog.getByRole("button", { name: "Create Folder" }).click();
	await expect(dialog).toBeHidden();
	await expect(fileNameCell(page, folderName)).toBeVisible({ timeout: 30_000 });
}

async function expectImagePreview(page: Page, fileName: string) {
	await fileNameCell(page, fileName).click();
	await chooseOpenMethodIfPrompted(page, "Image preview");
	const dialog = page.getByRole("dialog");
	await expect(dialog).toBeVisible();
	await expect(dialog.getByRole("img", { name: fileName })).toBeVisible({
		timeout: 30_000,
	});
}

async function expectPdfPreview(page: Page, fileName: string) {
	await fileNameCell(page, fileName).click();
	await chooseOpenMethodIfPrompted(page, "PDF preview");
	const dialog = page.getByRole("dialog");
	await expect(dialog).toBeVisible();
	await expect(dialog.getByText("Page 1 / 1")).toBeVisible({ timeout: 30_000 });
}

async function expectCodePreview(page: Page, fileName: string) {
	await fileNameCell(page, fileName).click();
	await chooseOpenMethodIfPrompted(page, "Source view");
	const dialog = page.getByRole("dialog");
	await expect(dialog).toBeVisible();
	await expect(
		dialog.getByText('const greeting = "Hello from Playwright";'),
	).toBeVisible({
		timeout: 30_000,
	});
}

async function closeActiveDialog(page: Page) {
	const dialog = page.getByRole("dialog");
	await page.keyboard.press("Escape");
	await expect(dialog).toBeHidden();
}

async function chooseOpenMethodIfPrompted(
	page: Page,
	label: string,
	timeoutMs = 0,
) {
	const chooser = page.getByRole("dialog", {
		name: "Choose how to open this file",
	});
	const chooserVisible =
		timeoutMs > 0
			? await chooser
					.waitFor({
						state: "visible",
						timeout: timeoutMs,
					})
					.then(() => true)
					.catch(() => false)
			: await chooser.isVisible().catch(() => false);

	if (!chooserVisible) {
		return;
	}

	let option = chooser.getByRole("button", { exact: true, name: label });
	if (!(await option.isVisible().catch(() => false))) {
		const more = chooser.getByRole("button", {
			exact: true,
			name: "More open methods",
		});
		if (await more.isVisible().catch(() => false)) {
			await more.click();
		}
		option = chooser.getByRole("button", { exact: true, name: label });
	}

	await option.click();
}

async function openItemContextMenu(page: Page, itemName: string) {
	await fileRow(page, itemName).click({ button: "right" });
}

async function renameItem(page: Page, currentName: string, nextName: string) {
	await openItemContextMenu(page, currentName);
	await page.getByRole("menuitem", { name: "Rename" }).click();

	const dialog = page.getByRole("dialog");
	await expect(dialog).toBeVisible();
	await dialog.getByRole("textbox").fill(nextName);
	await dialog.getByRole("button", { name: "Rename" }).click();
	await expect(dialog).toBeHidden();
}

async function chooseTargetFolder(
	page: Page,
	targetFolderName: string,
	confirmLabel: "Copy here" | "Move here",
) {
	const dialog = page.getByRole("dialog");
	await expect(dialog).toBeVisible();
	await dialog
		.getByRole("button", { exact: true, name: targetFolderName })
		.click();
	await dialog.getByRole("button", { name: confirmLabel }).click();
	await expect(dialog).toBeHidden();
}

async function copyItemToFolder(
	page: Page,
	itemName: string,
	targetFolderName: string,
) {
	await openItemContextMenu(page, itemName);
	await page.getByRole("menuitem", { name: "Copy" }).click();
	await chooseTargetFolder(page, targetFolderName, "Copy here");
}

async function moveItemToFolder(
	page: Page,
	itemName: string,
	targetFolderName: string,
) {
	await openItemContextMenu(page, itemName);
	await page.getByRole("menuitem", { name: "Move" }).click();
	await chooseTargetFolder(page, targetFolderName, "Move here");
}

async function deleteItem(page: Page, itemName: string) {
	await openItemContextMenu(page, itemName);
	await page.getByRole("menuitem", { name: "Delete" }).click();
}

async function toggleItemSelection(page: Page, itemName: string) {
	await fileRow(page, itemName).locator("button[aria-pressed]").first().click();
}

function folderTreeButton(page: Page, folderName: string) {
	return page
		.getByRole("complementary")
		.getByRole("button", { exact: true, name: folderName })
		.first();
}

async function openFolder(page: Page, folderName: string) {
	await expect(folderTreeButton(page, folderName)).toBeVisible({
		timeout: 30_000,
	});
	await folderTreeButton(page, folderName).click();
	await expect(
		page.getByRole("navigation", { name: "Breadcrumb" }).getByText(folderName, {
			exact: true,
		}),
	).toBeVisible();
}

async function navigateToRoot(page: Page) {
	await page.getByRole("link", { name: "My Drive" }).click();
	await expect(fileDropZone(page)).toBeVisible();
	await expect(page).toHaveURL(/\/$/);
}

async function expectItemMissing(page: Page, itemName: string) {
	await expect(fileNameCell(page, itemName)).toHaveCount(0, {
		timeout: 30_000,
	});
}

function trashItemRow(page: Page, itemName: string) {
	return page
		.getByRole("row")
		.filter({ has: page.getByText(itemName, { exact: true }) })
		.first();
}

async function expectTrashItemVisible(page: Page, itemName: string) {
	await expect(trashItemRow(page, itemName)).toBeVisible({ timeout: 30_000 });
}

async function expectTrashItemMissing(page: Page, itemName: string) {
	await expect(trashItemRow(page, itemName)).toHaveCount(0, {
		timeout: 30_000,
	});
}

async function expectDownloadMatches(
	page: Page,
	fileName: string,
	expected: Buffer,
	outputDir: string,
) {
	const downloadPromise = page.waitForEvent("download");
	await openItemContextMenu(page, fileName);
	await page.getByRole("menuitem", { name: "Download" }).click();

	const download = await downloadPromise;
	const targetPath = path.join(outputDir, download.suggestedFilename());
	await download.saveAs(targetPath);

	const actual = await readFile(targetPath);
	expect(actual.equals(expected)).toBe(true);
}

async function createPageShare(
	page: Page,
	itemName: string,
	options?: {
		password?: string;
	},
) {
	await openItemContextMenu(page, itemName);
	await page.getByRole("menuitem", { name: "Share page" }).click();

	const dialog = page.getByRole("dialog");
	await expect(dialog).toBeVisible();
	if (options?.password) {
		await dialog.getByLabel("Password (optional)").fill(options.password);
	}
	await dialog.getByRole("button", { name: "Create share link" }).click();

	const shareUrlInput = dialog.getByTestId("share-primary-url");
	await expect(shareUrlInput).toHaveValue(/\/s\//);
	const shareUrl = await shareUrlInput.inputValue();

	await dialog.getByRole("button", { name: "Done" }).click();
	await expect(dialog).toBeHidden();

	return shareUrl;
}

async function expectAnonymousSharePreview(
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

async function expectProtectedFolderSharePreview(
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
	await page.getByRole("button", { exact: true, name: fileName }).click();
	await chooseOpenMethodIfPrompted(page, "Image preview", 5_000);

	const dialog = page.getByRole("dialog");
	await expect(dialog.getByRole("img", { name: fileName })).toBeVisible({
		timeout: 30_000,
	});

	await closeActiveDialog(page);
	await context.close();
}

function uniqueName(prefix: string) {
	return `${prefix}-${RUN_ID}-${Math.random().toString(36).slice(2, 6)}`;
}

function uniqueAccountName(prefix: string) {
	return `${prefix}-${Math.random().toString(36).slice(2, 8)}`.slice(0, 16);
}

async function apiJsonInPage<T>(
	page: Page,
	requestPath: string,
	options?: {
		body?: unknown;
		method?: string;
		withCsrf?: boolean;
	},
) {
	const response = await page.evaluate(
		async ({ body, method, requestPath, withCsrf }) => {
			const readCookie = (name: string) => {
				const encodedName = `${encodeURIComponent(name)}=`;
				for (const chunk of document.cookie.split(";")) {
					const trimmed = chunk.trim();
					if (trimmed.startsWith(encodedName)) {
						return decodeURIComponent(trimmed.slice(encodedName.length));
					}
				}
				return null;
			};

			const headers: Record<string, string> = {};
			if (body !== undefined) {
				headers["Content-Type"] = "application/json";
			}
			if (withCsrf) {
				const token = readCookie("aster_csrf");
				if (token) {
					headers["X-CSRF-Token"] = token;
				}
			}

			const result = await fetch(requestPath, {
				body: body === undefined ? undefined : JSON.stringify(body),
				credentials: "include",
				headers,
				method,
			});

			return {
				status: result.status,
				text: await result.text(),
			};
		},
		{
			body: options?.body,
			method: options?.method ?? "GET",
			requestPath,
			withCsrf: options?.withCsrf ?? false,
		},
	);

	expect(response.status).toBeGreaterThanOrEqual(200);
	expect(response.status).toBeLessThan(300);
	const payload = JSON.parse(response.text) as {
		code: number;
		data: T;
		msg?: string;
	};
	expect(payload.code).toBe(0);
	return payload.data;
}

async function uploadChunkViaApi(
	page: Page,
	uploadId: string,
	chunkNumber: number,
	buffer: Buffer,
) {
	const response = await page.evaluate(
		async ({ bufferBase64, chunkNumber, uploadId }) => {
			const readCookie = (name: string) => {
				const encodedName = `${encodeURIComponent(name)}=`;
				for (const chunk of document.cookie.split(";")) {
					const trimmed = chunk.trim();
					if (trimmed.startsWith(encodedName)) {
						return decodeURIComponent(trimmed.slice(encodedName.length));
					}
				}
				return null;
			};
			const binary = atob(bufferBase64);
			const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
			const headers: Record<string, string> = {
				"Content-Type": "application/octet-stream",
			};
			const csrfToken = readCookie("aster_csrf");
			if (csrfToken) {
				headers["X-CSRF-Token"] = csrfToken;
			}

			const result = await fetch(
				`/api/v1/files/upload/${uploadId}/${chunkNumber}`,
				{
					body: bytes,
					credentials: "include",
					headers,
					method: "PUT",
				},
			);

			return {
				status: result.status,
				text: await result.text(),
			};
		},
		{
			bufferBase64: buffer.toString("base64"),
			chunkNumber,
			uploadId,
		},
	);

	expect(response.status).toBeGreaterThanOrEqual(200);
	expect(response.status).toBeLessThan(300);
	const payload = JSON.parse(response.text) as { code: number };
	expect(payload.code).toBe(0);
}

async function saveResumableSession(
	page: Page,
	session: {
		baseFolderId: number | null;
		baseFolderName: string;
		chunkSize: number;
		filename: string;
		mode: "chunked" | "presigned_multipart";
		relativePath: string | null;
		savedAt: number;
		totalChunks: number;
		totalSize: number;
		uploadId: string;
		workspace: { kind: "personal" } | { kind: "team"; teamId: number };
	},
) {
	await page.evaluate(
		({ session, storageKey }) => {
			const raw = window.localStorage.getItem(storageKey);
			const existing = raw ? (JSON.parse(raw) as unknown[]) : [];
			const next = existing.filter((item) => {
				if (
					typeof item === "object" &&
					item !== null &&
					"uploadId" in item &&
					typeof item.uploadId === "string"
				) {
					return item.uploadId !== session.uploadId;
				}
				return true;
			});
			next.push(session);
			window.localStorage.setItem(storageKey, JSON.stringify(next));
		},
		{
			session,
			storageKey: RESUMABLE_UPLOADS_KEY,
		},
	);
}

async function loadPersistedSessions(page: Page) {
	return page.evaluate((storageKey) => {
		const raw = window.localStorage.getItem(storageKey);
		return raw ? (JSON.parse(raw) as unknown[]) : [];
	}, RESUMABLE_UPLOADS_KEY);
}

function basicAuth(username: string, password: string) {
	return `Basic ${Buffer.from(`${username}:${password}`, "utf8").toString("base64")}`;
}

function normalizeWebdavPrefix(prefix: string) {
	const trimmed = prefix.trim();
	if (!trimmed) {
		return "/webdav";
	}

	if (trimmed === "/") {
		return "";
	}

	return trimmed.startsWith("/")
		? trimmed.replace(/\/+$/, "")
		: `/${trimmed.replace(/\/+$/, "")}`;
}

async function webdavRequest(
	page: Page,
	requestPath: string,
	options: {
		body?: string;
		headers?: Record<string, string>;
		method:
			| "DELETE"
			| "GET"
			| "MKCOL"
			| "PROPFIND"
			| "PUT";
	},
) {
	return page.evaluate(
		async ({ body, headers, method, requestPath }) => {
			const response = await fetch(requestPath, {
				body,
				headers,
				method,
			});
			return {
				status: response.status,
				text: await response.text(),
			};
		},
		{
			body: options.body,
			headers: options.headers,
			method: options.method,
			requestPath,
		},
	);
}
