# HTTP API

All endpoints are unauthenticated in the current version. Requests and responses use JSON unless noted.

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
- `400 Bad Request` if `id` is not a valid UUID. This response is plain text, not the JSON error format below.

## Errors

Errors from subnet endpoints use this format:

```json
{ "error": "not found" }
```

| Status | `error` | Meaning |
| --- | --- | --- |
| `404` | `not found` | The requested resource does not exist |
| `500` | `internal server error` | An unexpected server or database error. Details are written to the server log, never returned to the client. |
