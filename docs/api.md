# HTTP API

All endpoints are unauthenticated in the current version, including `POST /subnets` and `DELETE /subnets/{id}`, which write to the database. Anyone who can reach the server can create and delete subnets. Requests and responses use JSON unless noted.

## Health

### `GET /health`

Liveness check. Reports whether the process is running. Does not touch the database.

- `200 OK` with the plain-text body `ok`

### `GET /health/ready`

Readiness check. Reports whether the server can reach the database, by running `SELECT 1`.

- `200 OK` with the plain-text body `ready`
- `503 Service Unavailable` with the plain-text body `database unavailable`

Monitoring should restart the process only when `/health` fails. When only `/health/ready` fails, stop routing traffic to the server until it recovers; restarting will not fix an unreachable database.

## Subnets

### The subnet object

```json
{
  "id": "7a904281-76dd-4a32-a76c-f86f6bc0d839",
  "cidr": "10.0.1.0/24",
  "name": "Servers",
  "description": "",
  "vlan_id": 100,
  "parent_id": null,
  "created_at": "2026-09-22T02:41:36.123456Z",
  "updated_at": "2026-09-22T02:41:36.123456Z"
}
```

| Field | Type | Description |
| --- | --- | --- |
| `id` | string (UUID) | Unique identifier |
| `cidr` | string | Network in CIDR notation. Unique. |
| `name` | string | Display name |
| `description` | string | Free text. Empty string when not set. |
| `vlan_id` | integer or null | 802.1Q VLAN ID, 1 through 4094 |
| `parent_id` | string (UUID) or null | Containing subnet, if any |
| `created_at` | string | RFC 3339 timestamp, UTC |
| `updated_at` | string | RFC 3339 timestamp, UTC |

### `GET /subnets`

Lists all subnets, ordered by network address. There is no pagination or filtering yet.

- `200 OK` with an array of subnet objects

### `GET /subnets/{id}`

Returns one subnet.

- `200 OK` with a subnet object
- `404 Not Found` if no subnet has that ID
- `400 Bad Request` if `id` is not a valid UUID

### `POST /subnets`

Creates a subnet. The request body must be a JSON object sent with `Content-Type: application/json`.

```json
{
  "cidr": "10.0.1.0/24",
  "name": "Servers",
  "description": "Rack 4 application servers",
  "vlan_id": 100,
  "parent_id": null
}
```

| Field | Type | Required | Rules |
| --- | --- | --- | --- |
| `cidr` | string | Yes | An IPv4 or IPv6 network in CIDR notation. Host bits must be zero: `10.0.1.0/24` is accepted, `10.0.1.5/24` is rejected. Must not match the `cidr` of an existing subnet. |
| `name` | string | Yes | Leading and trailing whitespace is removed before validation and storage. After trimming, must be 1 to 100 characters. |
| `description` | string | No | At most 1000 characters. Stored as sent, without trimming. Defaults to an empty string. |
| `vlan_id` | integer or null | No | 1 through 4094. Defaults to null. |
| `parent_id` | string (UUID) or null | No | The ID of an existing subnet. Defaults to null. |

Character limits count Unicode characters, not bytes. Fields not listed above are rejected. The server does not check that the parent subnet's network contains the new subnet's network.

Rules are checked in the order `cidr`, `name`, `description`, `vlan_id`, and only the first failure is reported. The duplicate `cidr` and `parent_id` checks happen when the row is inserted, after the other rules pass.

- `201 Created` with the new subnet object
- `400 Bad Request` if a rule above fails, or the body is not valid JSON
- `409 Conflict` if a subnet with the same `cidr` already exists
- `415 Unsupported Media Type` if the `Content-Type` header is not `application/json`
- `422 Unprocessable Entity` if the body is valid JSON but does not match the request shape: a required field is missing, a field has the wrong type or an unparseable value, or an unknown field is present

Validation messages returned in `error`:

| Condition | `error` |
| --- | --- |
| `cidr` has host bits set | `cidr 10.0.1.5/24 has host bits set; the network address is 10.0.1.0/24` |
| `name` is empty after trimming | `name must not be empty` |
| `name` is too long | `name must be at most 100 characters` |
| `description` is too long | `description must be at most 1000 characters` |
| `vlan_id` is out of range | `vlan_id must be between 1 and 4094` |
| `parent_id` does not exist | `parent_id does not refer to an existing subnet` |
| `cidr` already exists | `a subnet with this cidr already exists` |

### `DELETE /subnets/{id}`

Deletes one subnet. A subnet that is the parent of other subnets cannot be deleted; delete its child subnets or reassign them to another parent first. Deletion does not cascade.

- `204 No Content` with no response body
- `404 Not Found` if no subnet has that ID
- `409 Conflict` if the subnet has child subnets
- `400 Bad Request` if `id` is not a valid UUID

Error messages returned in `error`:

| Condition | `error` |
| --- | --- |
| No subnet has that ID | `not found` |
| The subnet has child subnets | `subnet has child subnets; delete or reassign them first` |

## Errors

All endpoints except the health checks return errors as a JSON object with a single `error` field:

```json
{ "error": "not found" }
```

| Status | `error` | Meaning |
| --- | --- | --- |
| `400` | A description of the problem | A validation rule failed, the request body is not valid JSON, or a path parameter such as `{id}` could not be parsed |
| `404` | `not found` | The requested resource does not exist |
| `409` | A description of the conflict | The request conflicts with existing data |
| `415` | A description of the problem | The request body is not declared as JSON |
| `422` | A description of the problem | The request body does not match the expected shape |
| `500` | `internal server error` | An unexpected server or database error. Details are written to the server log, never returned to the client. |

For malformed request bodies and path parameters (the `400` body and path cases, `415`, and `422`), the status code and `error` text are produced by the Axum framework and may change when Axum is upgraded. Do not match on their exact wording.
