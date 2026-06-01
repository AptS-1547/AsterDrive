# Trash

The following paths are relative to `/api/v1` and require authentication.

## Overview

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/trash` | List trash contents |
| `POST` | `/trash/{entity_type}/{id}/restore` | Restore a file or folder |
| `DELETE` | `/trash/{entity_type}/{id}` | Permanently delete a file or folder |
| `DELETE` | `/trash` | Empty the current user's trash |

`entity_type` can only be `file` or `folder`.

`GET /trash` currently supports these pagination parameters:

- `folder_limit` / `folder_offset`
- `file_limit`
- `file_after_expires_at` / `file_after_id`

The response includes:

- `folders`
- `files`
- `folders_total`
- `files_total`
- `next_file_cursor`

So trash paging works like normal directory listing: folders use offset pagination, files use cursor pagination. Each trash item also includes `expires_at`, which is the automatic cleanup time computed from the current `trash_retention_days`.

## Restore and purge rules

- `GET /trash` returns the current user's trashed folders and files
- if the original parent folder no longer exists, restored items go back to the root
- restoring a folder also restores its deleted descendants
- `DELETE /trash/{entity_type}/{id}` performs permanent deletion
- `DELETE /trash` creates a `trash_purge_all` background task and returns `TaskInfo`

Permanent deletion updates blob reference counts, thumbnails, versions, and quota usage for files. Folders are removed recursively.

Implementation detail:

- the database-side deletion and blob `ref_count` decrement happen first
- physical storage and thumbnail cleanup happen after the transaction
- `file_blobs` metadata is removed only after the object is confirmed deleted
- if storage deletion fails temporarily, the blob metadata is kept and later retried by background maintenance

## Automatic cleanup

In addition to manual purge or permanent deletion, the system also removes expired items according to `trash_retention_days`. The periodic maintenance interval comes from `maintenance_cleanup_interval_secs` and includes jitter so multiple maintenance tasks do not all hit the database and storage at once.
