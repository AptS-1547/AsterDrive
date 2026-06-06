# Folders

The following paths are relative to `/api/v1` and require authentication.

## Endpoints

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/folders` | List root contents |
| `POST` | `/folders` | Create a folder |
| `GET` | `/folders/{id}` | List a folder's contents |
| `GET` | `/folders/{id}/info` | Read the full folder model |
| `GET` | `/folders/{id}/ancestors` | Read the breadcrumb ancestor chain |
| `PATCH` | `/folders/{id}` | Rename, move, or set the policy override |
| `DELETE` | `/folders/{id}` | Soft-delete a folder |
| `POST` | `/folders/{id}/lock` | Lock / unlock a folder |
| `POST` | `/folders/{id}/copy` | Recursively copy a folder |

## Directory listing

- `GET /folders`: read the root directory
- `GET /folders/{id}`: read a specific directory
- `GET /folders/{id}/info`: read the full model for one folder
- `GET /folders/{id}/ancestors`: return the ancestor chain for breadcrumbs

The REST folder listing returns real database rows. The API layer does not filter names such as `._*`, `~$*`, or `.DS_Store`.

`GET /folders` and `GET /folders/{id}` support:

- `folder_limit` / `folder_offset`
- `file_limit`
- `sort_by` / `sort_order`
- `file_after_value` / `file_after_id`

Folders use offset pagination, while files use cursor pagination.

Additional details:

- `folder_limit = 0` or `file_limit = 0` can be used to skip one side of the query
- `next_file_cursor` is returned for the file side
- the list endpoints intentionally trim some fields from `folders` / `files`; use `/folders/{id}/info` when you need the complete folder model
- `GET /folders/{id}/info` and `GET /teams/{team_id}/folders/{id}/info` include `storage_used`, the recursive quota-accounting bytes for all live files in the folder tree, including historical versions

## Create and update

Create request:

```json
{
  "name": "Documents",
  "parent_id": null
}
```

`parent_id = null` means create under the root.

`PATCH /folders/{id}` currently supports:

- rename
- move to another parent folder
- set a folder-level storage-policy override
- `parent_id = null` to move back to root
- `policy_id = null` to clear the folder-level policy override

Validation rules:

- a folder cannot be moved into itself or any descendant
- name conflicts at the destination are rejected
- locked folders cannot be modified

## Delete, lock, and copy

- `DELETE /folders/{id}` soft-deletes the folder and sends it to trash recursively
- `POST /folders/{id}/lock` toggles the lock state
- `POST /folders/{id}/copy` recursively copies the tree and returns `201`

When copying, the underlying blob content is not duplicated physically; only blob reference counts are incremented. Name conflicts at the destination are automatically resolved, and `parent_id = null` copies to the root. The copied subtree keeps the source folder's `policy_id`.
