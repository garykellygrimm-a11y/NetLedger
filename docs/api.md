# HTTP API

All endpoints are unauthenticated in the current version, including `POST /api/subnets`, `PATCH /api/subnets/{id}`, and `DELETE /api/subnets/{id}`, which write to the database. Anyone who can reach the server can create, change, and delete subnets. Requests and responses use JSON unless noted.

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
| `parent_id` | string (UUID) or null | The smallest other subnet that contains this one, or null for a top-level subnet. Set by the server; see [`POST /api/subnets`](#post-apisubnets). |
| `created_at` | string | RFC 3339 timestamp, UTC |
| `updated_at` | string | RFC 3339 timestamp, UTC |

### `GET /api/subnets`

Lists all subnets, ordered by network address. There is no pagination or filtering yet.

- `200 OK` with an array of subnet objects

### `GET /api/subnets/{id}`

Returns one subnet.

- `200 OK` with a subnet object
- `404 Not Found` if no subnet has that ID
- `400 Bad Request` if `id` is not a valid UUID

### `POST /api/subnets`

Creates a subnet. The request body must be a JSON object sent with `Content-Type: application/json`.

```json
{
  "cidr": "10.0.1.0/24",
  "name": "Servers",
  "description": "Rack 4 application servers",
  "vlan_id": 100
}
```

| Field | Type | Required | Rules |
| --- | --- | --- | --- |
| `cidr` | string | Yes | An IPv4 or IPv6 network in CIDR notation. Host bits must be zero: `10.0.1.0/24` is accepted, `10.0.1.5/24` is rejected. Must not match the `cidr` of an existing subnet. |
| `name` | string | Yes | Leading and trailing whitespace is removed before validation and storage. After trimming, must be 1 to 100 characters. |
| `description` | string | No | At most 1000 characters. Stored as sent, without trimming. Defaults to an empty string. |
| `vlan_id` | integer or null | No | 1 through 4094. Defaults to null. |

Character limits count Unicode characters, not bytes. Fields not listed above are rejected with `422`, including `parent_id`.

The server chooses the parent. It places the new subnet in the hierarchy in two steps:

1. The new subnet's parent is the smallest existing subnet that contains it. If no subnet contains it, it is created at the top level, with a null `parent_id`.
2. Existing subnets at that same level that lie inside the new subnet become its children. Subnets further down keep their parents.

Subnets can therefore be created in any order. For example, with top-level subnets `10.0.1.0/24` and `10.0.2.0/24`, creating `10.0.0.0/16` makes it their parent. Creating `10.0.0.0/8` afterwards makes it the parent of `10.0.0.0/16`, while `10.0.1.0/24` and `10.0.2.0/24` stay under `10.0.0.0/16`. Creating `10.0.3.0/24` then places it under `10.0.0.0/16`.

The response shows the new subnet's `parent_id`. It does not include the subnets that were moved under it. Their `parent_id` changes and their `updated_at` is set to the current time; fetch them again, for example with `GET /api/subnets`, to see the new hierarchy.

IPv4 and IPv6 networks never contain each other, so an IPv6 subnet is never placed under an IPv4 subnet, or the reverse.

The resulting hierarchy always follows two rules, which the database also enforces:

- Containment: a subnet with a `parent_id` lies inside the parent's network and is smaller than it.
- Overlap: subnets with the same parent do not overlap. Top-level subnets, those with a null `parent_id`, count as one level, so two top-level subnets do not overlap either.

Creates and deletes are processed one at a time, so two requests that arrive at the same moment cannot place subnets inconsistently. If two requests send the same `cidr`, one of them receives `409 Conflict`.

Checks run in this order, and only the first failure is reported:

1. The field rules in the table, in the order `cidr`, `name`, `description`, `vlan_id`.
2. The `cidr` must not match an existing subnet.

- `201 Created` with the new subnet object
- `400 Bad Request` if a field rule fails or the body is not valid JSON
- `409 Conflict` if a subnet with the same `cidr` already exists
- `415 Unsupported Media Type` if the `Content-Type` header is not `application/json`
- `422 Unprocessable Entity` if the body is valid JSON but does not match the request shape: a required field is missing, a field has the wrong type or an unparseable value, or an unknown field such as `parent_id` is present

Validation messages returned in `error`:

| Condition | `error` |
| --- | --- |
| `cidr` has host bits set | `cidr 10.0.1.5/24 has host bits set; the network address is 10.0.1.0/24` |
| `name` is empty after trimming | `name must not be empty` |
| `name` is too long | `name must be at most 100 characters` |
| `description` is too long | `description must be at most 1000 characters` |
| `vlan_id` is out of range | `vlan_id must be between 1 and 4094` |
| `cidr` is the same network as an existing subnet | `a subnet with this cidr already exists` |

### `PATCH /api/subnets/{id}`

Changes a subnet's `name`, `description`, and `vlan_id`. The request body must be a JSON object sent with `Content-Type: application/json`. It follows JSON Merge Patch semantics ([RFC 7396](https://www.rfc-editor.org/rfc/rfc7396)): a field that is omitted keeps its current value, a field with a value replaces the current value, and `"vlan_id": null` clears the VLAN.

For example, this request changes the description of the `Servers` subnet shown above and clears its VLAN, leaving its name unchanged:

```json
{
  "description": "Rack 5 application servers",
  "vlan_id": null
}
```

Response, `200 OK`:

```json
{
  "id": "7a904281-76dd-4a32-a76c-f86f6bc0d839",
  "cidr": "10.0.1.0/24",
  "name": "Servers",
  "description": "Rack 5 application servers",
  "vlan_id": null,
  "parent_id": null,
  "created_at": "2026-09-22T02:41:36.123456Z",
  "updated_at": "2026-09-29T14:05:12.654321Z"
}
```

| Field | Type | Rules |
| --- | --- | --- |
| `name` | string | Must not be null. Leading and trailing whitespace is removed before validation and storage. After trimming, must be 1 to 100 characters. |
| `description` | string | Must not be null; send an empty string to clear it. At most 1000 characters. Stored as sent, without trimming. |
| `vlan_id` | integer or null | 1 through 4094, or null to clear the VLAN. |

All fields are optional, but the request must include at least one of them; an empty object `{}` is rejected. A field sent with its current value counts as a change. Character limits count Unicode characters, not bytes.

`cidr` and `parent_id` cannot be changed through this endpoint. Sending either of them, or any other field not listed above, is rejected with `422`. The server sets `parent_id` when subnets are created and deleted; see [`POST /api/subnets`](#post-apisubnets) and [`DELETE /api/subnets/{id}`](#delete-apisubnetsid). Because this endpoint never changes `cidr` or `parent_id`, the hierarchy is not checked again.

Every successful request sets `updated_at` to the current time. `created_at` never changes.

Checks run in this order, and only the first failure is reported:

1. `id` must be a valid UUID.
2. The body must be JSON with the request shape.
3. The body must include at least one field, and then the field rules in the table are checked in the order `name`, `description`, `vlan_id`.
4. The subnet must exist.

A request with an invalid body for an ID that does not exist therefore returns `400` or `422`, not `404`.

- `200 OK` with the updated subnet object
- `400 Bad Request` if `id` is not a valid UUID, the body is empty (`{}`), a field rule fails, `name` or `description` is null, or the body is not valid JSON
- `404 Not Found` if no subnet has that ID
- `415 Unsupported Media Type` if the `Content-Type` header is not `application/json`
- `422 Unprocessable Entity` if the body is valid JSON but does not match the request shape: a field has the wrong type or an unparseable value, or an unknown field is present, including `cidr` and `parent_id`

Validation messages returned in `error`:

| Condition | `error` |
| --- | --- |
| The body includes none of the fields | `request must change at least one field` |
| `name` is null | `name must not be null` |
| `name` is empty after trimming | `name must not be empty` |
| `name` is too long | `name must be at most 100 characters` |
| `description` is null | `description must not be null; send an empty string to clear it` |
| `description` is too long | `description must be at most 1000 characters` |
| `vlan_id` is out of range | `vlan_id must be between 1 and 4094` |
| No subnet has that ID | `not found` |

### `DELETE /api/subnets/{id}`

Deletes one subnet. Its child subnets are not deleted; they move up to the deleted subnet's parent, or to the top level if the deleted subnet had no parent. Their `updated_at` is set to the current time. Subnets further down keep their parents.

For example, with `10.0.0.0/8` containing `10.0.0.0/16`, which contains `10.0.1.0/24`, deleting `10.0.0.0/16` makes `10.0.0.0/8` the parent of `10.0.1.0/24`.

- `204 No Content` with no response body
- `404 Not Found` if no subnet has that ID
- `400 Bad Request` if `id` is not a valid UUID

Error messages returned in `error`:

| Condition | `error` |
| --- | --- |
| No subnet has that ID | `not found` |

## Web UI

The server serves the web UI from the same address as the API. Requests to any path other than the health checks and paths under `/api` are handled as web UI requests. The handler does not check the request method. The leading `/` is removed from the path, and then:

1. If the path names a file in the built front end, `web/dist`, the file is returned with a `Content-Type` based on its extension. Files under `assets/` are sent with `Cache-Control: public, max-age=31536000, immutable`, and all other files with `Cache-Control: no-cache`.
2. Otherwise, if the path contains a period (`.`) anywhere, it is treated as a missing file and returns `404 Not Found` with an empty body.
3. Otherwise, the path is treated as a page route and returns `index.html` with `200 OK`, so the web UI handles the route in the browser. `/` is handled this way.

If the server was built without the front end, page routes return `404 Not Found` with a plain-text message explaining that the web front end is not included in the build. See [CONTRIBUTING.md](../CONTRIBUTING.md#web-front-end) for how builds include it.

## Security headers

Every response, including API, health check, and web UI responses, carries these headers. They replace any value a handler set.

| Header | Value |
| --- | --- |
| `Content-Security-Policy` | See below |
| `X-Content-Type-Options` | `nosniff` |
| `X-Frame-Options` | `DENY` |
| `Referrer-Policy` | `no-referrer` |

The `Content-Security-Policy` value is:

```text
default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'
```

It limits scripts, styles, fonts, connections, and form submissions to the server's own origin. Images may also use `data:` URLs. Plugins are blocked, and no site, including NetLedger itself, may embed its pages in a frame.

## Errors

The health checks and the web UI return plain-text or empty error bodies, described above. Everything under `/api/`, including a request to a path that matches no endpoint, returns errors as a JSON object with a single `error` field:

```json
{ "error": "not found" }
```

| Status | `error` | Meaning |
| --- | --- | --- |
| `400` | A description of the problem | A validation rule failed, the request body is not valid JSON, or a path parameter such as `{id}` could not be parsed |
| `404` | `not found` | The requested resource does not exist, or no endpoint matches a path under `/api/` |
| `409` | A description of the conflict | The request conflicts with existing data |
| `415` | A description of the problem | The request body is not declared as JSON |
| `422` | A description of the problem | The request body does not match the expected shape |
| `500` | `internal server error` | An unexpected server or database error. Details are written to the server log, never returned to the client. |

For malformed request bodies and path parameters (the `400` body and path cases, `415`, and `422`), the status code and `error` text are produced by the Axum framework and may change when Axum is upgraded. Do not match on their exact wording.
