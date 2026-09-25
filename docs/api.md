# HTTP API

All endpoints are unauthenticated in the current version, including `POST /api/subnets` and `DELETE /api/subnets/{id}`, which write to the database. Anyone who can reach the server can create and delete subnets. Requests and responses use JSON unless noted.

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
  "vlan_id": 100,
  "parent_id": null
}
```

| Field | Type | Required | Rules |
| --- | --- | --- | --- |
| `cidr` | string | Yes | An IPv4 or IPv6 network in CIDR notation. Host bits must be zero: `10.0.1.0/24` is accepted, `10.0.1.5/24` is rejected. Must not match the `cidr` of an existing subnet. Must also satisfy the hierarchy rules below. |
| `name` | string | Yes | Leading and trailing whitespace is removed before validation and storage. After trimming, must be 1 to 100 characters. |
| `description` | string | No | At most 1000 characters. Stored as sent, without trimming. Defaults to an empty string. |
| `vlan_id` | integer or null | No | 1 through 4094. Defaults to null. |
| `parent_id` | string (UUID) or null | No | The ID of an existing subnet. Defaults to null. |

Character limits count Unicode characters, not bytes. Fields not listed above are rejected.

Every subnet must also follow two hierarchy rules:

- Containment: a subnet with a `parent_id` must lie inside the parent's network and be smaller than it. `10.0.1.0/24` can be a child of `10.0.0.0/16`; `192.168.50.0/24` and `10.0.0.0/16` itself cannot.
- Overlap: subnets with the same parent must not overlap. Top-level subnets, those with a null `parent_id`, count as one level, so two top-level subnets must not overlap either.

Two CIDR networks can never partly overlap. Either they share no addresses, or one contains the other, or they are the same network. An overlap at the same level is therefore always one of three cases, each with its own message:

- The new `cidr` is the same network as an existing subnet.
- The new `cidr` is inside an existing subnet. With `10.0.0.0/16` at the top level, a second top-level `10.0.5.0/24` is rejected; create it with `10.0.0.0/16` as its parent instead.
- The new `cidr` contains an existing subnet. With `10.0.0.0/16` at the top level, a top-level `10.0.0.0/8` is rejected. Subnets cannot be moved to a new parent, so create larger subnets before the subnets inside them.

IPv4 and IPv6 networks never overlap each other. An IPv6 subnet cannot be a child of an IPv4 subnet, or the reverse, because it is not inside the parent's network.

Checks run in this order, and only the first failure is reported:

1. The field rules in the table, in the order `cidr`, `name`, `description`, `vlan_id`.
2. If `parent_id` is set, the parent must exist, and then the containment rule is checked against it.
3. The overlap rule is checked against the other subnets at the same level. If the new `cidr` contains more than one of them, the message names only one.
4. The row is inserted.

A child with the same network as its parent fails at step 2 with `400`, not with the duplicate `cidr` message.

The database enforces the same rules with a trigger and constraints. Two requests that arrive at the same moment can both pass steps 2 and 3, for example two overlapping top-level subnets. The database then rejects one of them at step 4 with `409 Conflict` and a generic message: `cidr overlaps another subnet at the same level`, or `a subnet with this cidr already exists` if the two networks are the same. If the parent is deleted between step 2 and step 4, the response is `400` with `parent_id does not refer to an existing subnet`. The generic `400` message `cidr must be inside the parent subnet's network` comes from the database's containment trigger. The API never changes a subnet's `cidr`, so this message should appear only if the database is modified outside the API between step 2 and step 4.

- `201 Created` with the new subnet object
- `400 Bad Request` if a field rule fails, `parent_id` does not exist, the `cidr` is not inside the parent's network, or the body is not valid JSON
- `409 Conflict` if the `cidr` overlaps another subnet at the same level, including the same network
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
| `cidr` is not inside the parent's network | `cidr <cidr> is not inside the parent subnet <parent cidr> (<parent name>)` |
| `cidr` is the same network as a subnet at the same level | `a subnet with this cidr already exists` |
| `cidr` is inside a subnet at the same level | `cidr <cidr> is inside the existing subnet <existing cidr> (<existing name>); choose it as the parent` |
| `cidr` contains a subnet at the same level | `cidr <cidr> contains the existing subnet <existing cidr> (<existing name>) at the same level; create larger subnets before the subnets inside them` |
| Containment rejected by the database (see above) | `cidr must be inside the parent subnet's network` |
| Overlap rejected by the database for simultaneous requests | `cidr overlaps another subnet at the same level` |

Placeholders in angle brackets are filled in from the request and the existing subnet. For example, with a top-level subnet `10.0.0.0/16` named `Datacenter`, a top-level `10.0.5.0/24` returns:

```json
{ "error": "cidr 10.0.5.0/24 is inside the existing subnet 10.0.0.0/16 (Datacenter); choose it as the parent" }
```

A child `192.168.50.0/24` of the subnet `10.0.0.0/16` named `Parent` returns `400` with `cidr 192.168.50.0/24 is not inside the parent subnet 10.0.0.0/16 (Parent)`.

### `DELETE /api/subnets/{id}`

Deletes one subnet. A subnet that is the parent of other subnets cannot be deleted; delete its child subnets first. Deletion does not cascade.

- `204 No Content` with no response body
- `404 Not Found` if no subnet has that ID
- `409 Conflict` if the subnet has child subnets
- `400 Bad Request` if `id` is not a valid UUID

Error messages returned in `error`:

| Condition | `error` |
| --- | --- |
| No subnet has that ID | `not found` |
| The subnet has child subnets | `subnet has child subnets; delete them first` |

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
