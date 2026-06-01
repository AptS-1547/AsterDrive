# Properties

Entity properties are custom key-value pairs attached to files or folders.

The following paths are relative to `/api/v1` and require authentication.

## Endpoints

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/properties/{entity_type}/{entity_id}` | List entity properties |
| `PUT` | `/properties/{entity_type}/{entity_id}` | Create or update a property |
| `DELETE` | `/properties/{entity_type}/{entity_id}/{namespace}/{name}` | Delete a property |

`entity_type` can only be:

- `file`
- `folder`

## `GET /properties/{entity_type}/{entity_id}`

Returns all properties attached to that entity.

## `PUT /properties/{entity_type}/{entity_id}`

Request body:

```json
{
  "namespace": "custom",
  "name": "color",
  "value": "blue"
}
```

`value` may be `null`.

## `DELETE /properties/{entity_type}/{entity_id}/{namespace}/{name}`

Deletes the specified property.

## Read-only namespace

The REST API does not allow modifying the `DAV:` namespace.

In practice:

- `namespace = "DAV:"` cannot be `PUT`
- `namespace = "DAV:"` cannot be `DELETE`

This keeps the REST API from breaking WebDAV protocol properties.
