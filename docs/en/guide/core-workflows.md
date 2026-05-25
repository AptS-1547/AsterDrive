# Common Workflows

This page organizes AsterDrive's daily actions by **real scenarios**: what to check after a new deployment, how to connect Office files to online open methods, how to assign storage routes to different users, and how to recover after mistaken deletion.  
If you do not want to read the whole manual, jump to the scenario you need and follow it.

## First Admin Check After a New Deployment

The full checklist is in [First-Start Checklist](/en/deployment/runtime-behavior). That page covers, in order, what was completed automatically and what you should check manually after startup.

This `Common Workflows` page adds one thing: after your first login, the fastest admin path is to go through these **admin entries in order**:

1. `Admin -> Overview`
2. `Admin -> Users`
3. `Admin -> Teams`
4. `Admin -> Storage Policies`
5. `Admin -> Policy Groups`
6. `Admin -> Tasks`
7. `Admin -> System Settings`
8. Return to your personal space, then open left-side `WebDAV`

If you plan to connect follower nodes, also check `Admin -> Follower Nodes`.

For what to confirm in each item, [First-Start Checklist](/en/deployment/runtime-behavior#check-these-items-immediately-after-startup) lists more detail.

## Add Online Open Methods for Office Files

If you plan to hand files such as `docx`, `xlsx`, and `pptx` to an external service, the recommended flow is:

1. Set `Admin -> System Settings -> Site Configuration -> Public site URL` correctly
2. Enable or import the corresponding WOPI app in `Admin -> System Settings -> Site Configuration -> Preview Apps`
3. Confirm the external Office / WOPI service can reach AsterDrive's `/api/v1/wopi/...`
4. If the browser console clearly reports an AsterDrive API CORS error, allow the corresponding origin in `Admin -> System Settings -> Network Access`
5. Open and save a real Office file once

## Upload and Organize Files

When organizing materials, the usual order is:

1. Create the base folders first
2. Upload files or folders
3. Drag them into the target directories
4. Use multi-select for batch move, copy, or delete

## Assign Storage Routes to Different Users or Teams

If you want different users or teams to use different storage locations, the recommended flow is:

1. Create real storage targets in `Admin -> Storage Policies`
2. Define file-size routing rules in `Admin -> Policy Groups`
3. Bind the corresponding policy group in `Admin -> Users` or `Admin -> Teams`

Common patterns:

- Everyone uses local storage by default
- Some teams use S3 / MinIO separately
- Some teams use follower-node storage separately
- Small files use local storage, large files use object storage

If the route includes a follower node, first confirm that follower already has a default ingress target.  
Without an ingress target, the remote storage policy itself can be saved, but actual uploads will be rejected by the follower.

## Prepare an Ingress Target for a Follower Node

Follower nodes currently require two steps:

1. Create a node in `Admin -> Follower Nodes`, generate the enroll command, and connect the follower to the primary
2. Return to this remote node's details page and create the default ingress target

For the first attempt, use a `local` ingress target:

- Set the base path to a relative path such as `default`
- Check default ingress target
- After the status becomes applied, create the remote storage policy

If the follower will ultimately write objects to S3 / MinIO, create an `s3` ingress target there. Do not pass it through the enroll command or Docker bootstrap ENV.

## Create a Team Space

In the current version, teams are created by system administrators.  
Recommended order:

1. Create the team in `Admin -> Teams`
2. Choose the initial team administrator
3. Select the policy group bound to this team
4. Let members enter the team space from the left-side workspace list

After the team is created, team administrators or owners can continue managing members and viewing team audit from `Settings -> Teams`.

## Send Materials to Others

When sending materials, usually:

1. Switch to the correct workspace first
2. Create a share link on the file or folder
3. Set a password if confidentiality is needed
4. Set an expiration time if time control is needed
5. Set a download limit if spread should be limited
6. Copy the link in `My Shares` and check that the public page works

If you only change the password, expiration time, or download count, edit the existing link in `My Shares` directly.

## Continue an Unfinished Upload

After a large-file upload is interrupted, the usual flow is:

1. Return to the original folder
2. Find the unfinished upload task
3. Select the same file again

As long as the upload session has not expired, AsterDrive tries to continue the unfinished parts instead of starting from scratch.

## Create a WebDAV Account for a Device

If you want to connect AsterDrive through Finder, Windows Explorer, rclone, or a sync tool, the recommended flow is:

1. Switch back to `My Space`, then create a dedicated account on the `WebDAV` page
2. Restrict it to a directory if needed
3. Fill the WebDAV address, username, and password into the client
4. Run a real read/write test first

When a device is retired, disabling only that WebDAV account will not affect the web login password.

## Handle Mistaken Deletion

Items deleted normally enter the trash first.  
The most common flow is:

1. Go directly to the trash and restore after mistaken deletion
2. Permanently delete items that are no longer needed from the trash
3. Administrators regularly confirm that trash retention days are reasonable

## Handle Lock Problems

If a file keeps showing as locked, the common order is:

1. Ask the person editing it to save and exit normally
2. If a WebDAV client exited abnormally, check `Admin -> Locks`
3. After confirming the lock has expired, let an administrator clean it manually

## Thumbnail Generation Is Not as Expected

If images, videos, or special formats never get thumbnails, administrators should check in this order:

1. Whether `Admin -> Tasks` has failed thumbnail tasks
2. Whether the corresponding processor is enabled in `Admin -> System Settings -> Storage and Retention -> Media Processing`
3. Whether the target extension is bound to the correct processor
4. If `vips_cli` or `ffmpeg_cli` is used, whether the test command passes
5. Whether the source file exceeds the thumbnail source file size limit
