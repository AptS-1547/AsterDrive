# File Editing

This page explains **how to edit files directly in AsterDrive**, including in-browser editing for text files and integration flows that hand Office files to external editors such as Collabora / OnlyOffice.

AsterDrive currently has two common editing paths:

- Edit text files directly in the browser
- Use WOPI open methods configured by an administrator to hand Office-like files to external online editors

You usually do not need to distinguish the underlying implementation first. After opening a file, whether it can be edited directly and whether extra "open with" entries appear are determined by the file type and current administrator configuration.

## What Is Suitable for Direct Browser Editing

In-page editing mainly targets text files, such as:

- Markdown
- CSV / TSV
- JSON
- XML
- Configuration files such as TOML, YAML, and INI
- Logs
- Scripts
- Common code files

This path is best for documents, configuration, scripts, and source code.  
For Office files such as Word, Excel, and PowerPoint, it usually depends on whether the administrator configured WOPI open methods for the site.

## What You See After Opening a File

### Text Files

Text files can usually be opened and saved directly on the web page.  
When saving, AsterDrive automatically:

- Locks the file during editing
- Checks before saving whether someone else has changed the file
- Creates a new version after successful save
- Releases the lock after the editor closes

If you receive a conflict prompt while saving, it usually means someone else changed the file during your edit.  
Refresh the content first, then decide whether to continue.

### Office Files and Other External Open Methods

If the administrator configured preview apps, some files will show additional "open with" options.  
These entries may include:

- Built-in previewer
- External URL template previewer
- WOPI online open method

The most common use of WOPI is handing files such as `docx`, `xlsx`, and `pptx` to compatible services such as OnlyOffice.  
Whether it opens in the current dialog or a new tab depends on how the administrator configured that open method.

A URL template previewer is more like "handing the current file's preview link to an external web page".  
The built-in Microsoft / Google previewers belong to this category. They usually require the file preview link to be reachable by the external service. Intranet addresses, `localhost`, or plain HTTP links often fail directly.

If an Office file does not show any extra entry, there are usually only two reasons:

- The administrator has not configured a corresponding preview app for this file type
- The current deployment has no usable WOPI service connected

## What WOPI Means for Users

From a user's perspective, remember these points about WOPI:

- It is not a separate site entry; it is an open method inside the file preview window
- Not every deployment has it
- AsterDrive creates a temporary session and issues an access token when you open the file
- Content saved back through WOPI is written back to the original file
- WOPI overwrite saves also enter version history

If the administrator connects an external Office service, the editing interface style and buttons you see are mainly decided by that service, not entirely by AsterDrive.

## When Version History Is Created

A version is created whenever an overwrite write happens. Common sources include:

- In-browser text editing
- WOPI online save
- WebDAV overwrite save
- Other write paths that directly overwrite the original file content

In version history, you can:

- View old versions
- Restore a version
- Delete a version

After restoring an old version, versions newer than it are truncated together, so confirm before restoring.

## WebDAV Editing Also Keeps Versions

If you prefer desktop applications, you can also edit files through WebDAV.  
When WebDAV overwrites a file, it also creates a version.

Common scenarios include:

- Finder mounted network location
- Windows mapped network drive
- rclone or other sync tools
- Editors or office software that support WebDAV

## What Administrators Must Configure Before WOPI Appears

If you are an administrator, the WOPI-related entries are mainly here:

- `Admin -> System Settings -> Site Configuration -> Public site URL`
- `Admin -> System Settings -> Site Configuration -> Preview Apps`

The most common preparation order is:

1. Set `Public site URL` to the HTTP(S) origin users actually use to access AsterDrive; add each public entry point separately
2. Import or create WOPI open methods in `Preview Apps`
3. Confirm the external Office / WOPI service can access `/api/v1/wopi/...` generated from `Public site URL`
4. If the browser console clearly reports an AsterDrive API CORS error, allow the corresponding origin in `Admin -> System Settings -> Network Access`
5. Open a real Office file once, then confirm it can save back to AsterDrive normally

If you use `WOPI Discovery` in `Preview Apps`, the system generates corresponding apps from the discovery address.  
If the entry appears but opening fails later, first check the discovery address, public site URL, and network connectivity from the WOPI service to AsterDrive.

## What to Do About Lock Problems

While a file is being edited, other users cannot freely overwrite, rename, move, or delete it.

Common handling:

- Ask the lock holder to save and exit normally first
- If a browser tab, WebDAV client, or external WOPI editor interrupted abnormally, ask an administrator to clean the remaining lock in `Admin -> Locks`

## Boundaries

- In-browser editing is mainly for text files
- Whether Office-like files can open online depends on whether the administrator configured corresponding preview apps
- Whether WOPI supports multi-user collaboration depends on the external service you connect; AsterDrive handles file access, sessions, and locks
- AsterDrive does not merge conflicts automatically
- The administrator controls how many historical versions are retained
