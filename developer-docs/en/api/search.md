# Search

The following paths are relative to `/api/v1` and require authentication.

## Endpoints

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/search` | Search files and folders in the current user's personal space |
| `GET` | `/teams/{team_id}/search` | Search files and folders in the specified team workspace |

## Query parameters

Common parameters:

- `q`: fuzzy name match, case-insensitive
- `type`: `file`, `folder`, or `all`, default `all`
- `mime_type`: exact MIME filter for files
- `category`: file category filter, supports `image`, `video`, `audio`, `document`, `spreadsheet`, `presentation`, `archive`, `code`, `other`
- `extensions`: file-extension filter, preferably a comma-separated string such as `pdf,docx,tar.gz`
- `min_size` / `max_size`: file-size filter
- `created_after` / `created_before`: RFC3339 timestamps
- `folder_id`: narrow the search scope to one folder
- `limit`: per-resource-type cap, default `50`, max `100`
- `offset`: offset

Validation rules:

- `created_after` / `created_before` must be valid RFC3339 timestamps, or the API returns `400`
- If both are present, `created_after <= created_before` must hold
- `category` must be one of the listed values
- `extensions` cannot be empty, cannot contain empty segments, and cannot contain path separators or invalid characters
- Extensions are normalized to lower-case without a leading dot
- `category` and `extensions` are file-only filters; if they are sent together with `type=folder`, the API returns `400`

## Response shape

The response contains two result sets:

- `files`
- `folders`
- `total_files`
- `total_folders`

The `files` and `folders` items reuse the list-item shape from the regular listing APIs, so they include state such as `is_locked` and `is_shared`. File items also include `extension`, `compound_extension`, and `file_category`.

## Current semantics

- `/search` searches only the current user's personal space
- `/teams/{team_id}/search` first verifies team access, then searches that workspace
- items already in trash are omitted
- `type=folder` never returns files
- `type=file` never returns folders
- `folder_id` filters files by `folder_id` and folders by `parent_id`
- `category` prefers the persisted extension-based classification, and falls back to MIME only when needed
- compound extensions are persisted only for explicitly supported suffixes such as `tar.gz`
