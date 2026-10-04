# HTTP API

Every endpoint under `/api` except signing in requires a signed-in account, either through a browser session or an API token. Requests and responses use JSON unless noted.

## Authentication

NetLedger authenticates accounts. A browser signs in with a username and password and receives a session cookie; scripts and pipelines use API tokens. The first account is created with the `create-admin` command described in [Configuration](configuration.md#create-admin-username); there are no default credentials.

### Roles

Every account has one role, and every endpoint names the role it needs:

| Role | Can |
| --- | --- |
| `viewer` | Read subnets and addresses |
| `editor` | Everything a viewer can, and create, change, and delete subnets and addresses |
| `administrator` | Everything an editor can. Account management is not available through the API yet. |

Any signed-in account can manage its own API tokens. A request without valid credentials gets `401 Unauthorized`. A request from an account whose role is not sufficient gets `403 Forbidden`. The role check runs before the request body is read, so a viewer sending an invalid body to a write endpoint gets `403`, not `400`.

### Browser sessions

Signing in sets a cookie named `netledger_session`. It is `HttpOnly` (not readable by scripts), `SameSite=Strict` (never sent on requests that start on another site), and `Secure` unless [`NETLEDGER_COOKIE_SECURE`](configuration.md#netledger_cookie_secure) is `false`. The cookie's value is a random token; the server stores only a hash of it.

A session ends when any of these happen:

- `DELETE /api/session` is called.
- 12 hours have passed since sign-in.
- 30 minutes have passed since the session's last request.
- The account is disabled.

After that, the next request gets `401` and the client must sign in again. The web UI returns to the sign-in page when that happens.

Every response under `/api` carries `Cache-Control: no-store`, so browsers and proxies never cache data that was served to one account.

### API tokens

An API token lets a script act as the account that created it, with that account's role. Send it in the `Authorization` header:

```text
Authorization: Bearer nlt_<43 characters>
```

Tokens begin with `nlt_` so secret scanners can recognize a leaked one. A token's value is returned once, when it is created; NetLedger stores only a SHA-256 hash of it, so a lost token cannot be recovered, only revoked and replaced. A token stops working when it is revoked, when it expires, or when its account is disabled. When a request carries both a token and a session cookie, the token is used.

Tokens can read and change subnets and addresses like any other credential, but three actions require a password session: creating tokens, revoking tokens, and signing out. A token that tries one of them gets `403` with `this action requires signing in with a password, not an API token`. This means a leaked token cannot be used to mint more tokens.

Every change made with a token is recorded in the audit log with the token's ID, so changes can be traced to the token that made them, not only to its account.

PowerShell:

```powershell
$headers = @{ Authorization = "Bearer $env:NETLEDGER_TOKEN" }
Invoke-RestMethod -Uri https://netledger.example.internal/api/subnets -Headers $headers
```

curl:

```bash
curl -H "Authorization: Bearer $NETLEDGER_TOKEN" https://netledger.example.internal/api/subnets
```

Keep tokens in a secret store or an environment variable, never in a script or a command line that is saved to history.

### Signing in from a script

For interactive scripts, a password session also works. PowerShell keeps the cookie in a session variable:

```powershell
$credential = Get-Credential
$body = @{ username = $credential.UserName; password = $credential.GetNetworkCredential().Password } | ConvertTo-Json
Invoke-WebRequest -Method Post -Uri https://netledger.example.internal/api/session `
    -ContentType application/json -Body $body -SessionVariable session | Out-Null
Invoke-RestMethod -Uri https://netledger.example.internal/api/subnets -WebSession $session
```

### `POST /api/session`

Signs in. This is the only endpoint under `/api` that does not require credentials.

```json
{
  "username": "gary",
  "password": "correct horse battery staple"
}
```

The username is case-insensitive and surrounding whitespace is ignored. The password is compared exactly, after Unicode normalization.

- `201 Created` with the current-user object below, and a `Set-Cookie` header carrying the session cookie
- `401 Unauthorized` with `authentication required` if the username does not exist, the password is wrong, or the account is disabled. The three cases are indistinguishable from the response, by design.
- `429 Too Many Requests` with `too many failed attempts; try again later` and a `Retry-After` header giving the number of seconds to wait. This happens after 10 failed attempts for one username, or 50 failed attempts from one client address, within 15 minutes. The limit applies even when the password is correct. Each failed attempt, including ones refused by the limit, is recorded in the audit log with its reason.

### `GET /api/session`

Returns the account behind the request's credentials, whether a session cookie or an API token.

```json
{
  "account_id": "3f0f2b0e-5d4a-4c9b-9a6e-2d1c0b7a8f11",
  "username": "gary",
  "role": "administrator"
}
```

- `200 OK` with the current-user object
- `401 Unauthorized` if there are no valid credentials

### `DELETE /api/session`

Signs out: deletes the session on the server and clears the cookie. Requires a password session.

- `204 No Content`
- `401 Unauthorized` if there was no valid session to end
- `403 Forbidden` if the request used an API token

### The token object

```json
{
  "id": "c7e6a1f2-1b8d-4c0e-9a3f-5d2b7e4f8a10",
  "name": "Provisioning pipeline",
  "hint": "BRBQ",
  "created_at": "2026-10-04T23:12:11.123456Z",
  "expires_at": null,
  "last_used_at": "2026-10-05T08:00:02.551233Z",
  "revoked_at": null
}
```

| Field | Type | Description |
| --- | --- | --- |
| `id` | string (UUID) | Unique identifier |
| `name` | string | A label chosen when the token was created |
| `hint` | string | The last four characters of the token, to tell tokens apart |
| `created_at` | string | RFC 3339 timestamp, UTC |
| `expires_at` | string or null | When the token stops working, or null if it never expires |
| `last_used_at` | string or null | When the token last authenticated a request, updated at most once a minute; null if never used |
| `revoked_at` | string or null | When the token was revoked, or null if it is still active |

### `GET /api/tokens`

Role: any. Lists the requesting account's tokens, newest first, including revoked and expired ones. Tokens belonging to other accounts are never listed, even for administrators.

- `200 OK` with an array of token objects

### `POST /api/tokens`

Role: any, with a password session. Creates a token for the requesting account.

```json
{
  "name": "Provisioning pipeline",
  "expires_in_days": 90
}
```

| Field | Required | Rules |
| --- | --- | --- |
| `name` | Yes | 1 to 100 characters after trimming whitespace |
| `expires_in_days` | No | 1 through 365. Omit it for a token that never expires. |

- `201 Created` with the token object plus a `secret` field holding the token itself. This is the only time the token's value is returned.
- `400 Bad Request` if a rule above is broken
- `403 Forbidden` if the request used an API token
- `422 Unprocessable Entity` if the body has unknown fields

### `DELETE /api/tokens/{id}`

Role: any, with a password session. Revokes one of the requesting account's tokens. The token stays in the list with `revoked_at` set.

- `204 No Content`
- `403 Forbidden` if the request used an API token
- `404 Not Found` if the token does not exist, belongs to another account, or is already revoked

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

Role: viewer. Lists all subnets, ordered by network address. There is no pagination or filtering yet.

- `200 OK` with an array of subnet objects

### `GET /api/subnets/{id}`

Role: viewer. Returns one subnet.

- `200 OK` with a subnet object
- `404 Not Found` if no subnet has that ID
- `400 Bad Request` if `id` is not a valid UUID

### `POST /api/subnets`

Role: editor. Creates a subnet. The request body must be a JSON object sent with `Content-Type: application/json`.

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

Recorded addresses move in the same way. If the new subnet has a parent, addresses recorded in that parent that lie inside the new subnet move to the new subnet. Addresses recorded in other subnets, including the new subnet's children, keep their subnet. If a moved address would be the new subnet's network or broadcast address, which IPv4 subnets of /30 or shorter reserve, the subnet is not created and the request returns `409 Conflict`. For example, with the address `10.0.1.0` recorded in `10.0.0.0/16`, creating `10.0.1.0/24` is rejected because `10.0.1.0` would be its network address. See [Addresses](#addresses).

The response shows the new subnet's `parent_id`. It does not include the subnets that were moved under it or the addresses that were moved into it. Their `parent_id` or `subnet_id` changes and their `updated_at` is set to the current time; fetch them again, for example with `GET /api/subnets` and [`GET /api/subnets/{id}/addresses`](#get-apisubnetsidaddresses), to see the result.

IPv4 and IPv6 networks never contain each other, so an IPv6 subnet is never placed under an IPv4 subnet, or the reverse.

The resulting hierarchy always follows two rules, which the database also enforces:

- Containment: a subnet with a `parent_id` lies inside the parent's network and is smaller than it.
- Overlap: subnets with the same parent do not overlap. Top-level subnets, those with a null `parent_id`, count as one level, so two top-level subnets do not overlap either.

Creating and deleting subnets, recording addresses, and allocating addresses are serialized with a shared lock, so their placement decisions never interleave. Deleting an address and the other operations do not take that lock. They rely on the database's normal row-level locking, which is safe because they make no placement decisions. If two requests send the same `cidr`, one of them receives `409 Conflict`.

Checks run in this order, and only the first failure is reported:

1. The field rules in the table, in the order `cidr`, `name`, `description`, `vlan_id`.
2. The `cidr` must not match an existing subnet.
3. No recorded address that would move into the new subnet may be its network or broadcast address.

- `201 Created` with the new subnet object
- `400 Bad Request` if a field rule fails or the body is not valid JSON
- `409 Conflict` if a subnet with the same `cidr` already exists, or a recorded address would become the new subnet's network or broadcast address
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
| A recorded address would become the network or broadcast address | `address 10.0.1.0 is recorded and would become the network or broadcast address of 10.0.1.0/24` |

### `PATCH /api/subnets/{id}`

Role: editor. Changes a subnet's `name`, `description`, and `vlan_id`. The request body must be a JSON object sent with `Content-Type: application/json`. It follows JSON Merge Patch semantics ([RFC 7396](https://www.rfc-editor.org/rfc/rfc7396)): a field that is omitted keeps its current value, a field with a value replaces the current value, and `"vlan_id": null` clears the VLAN.

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

Role: editor. Deletes one subnet. Its child subnets are not deleted; they move up to the deleted subnet's parent, or to the top level if the deleted subnet had no parent. Their `updated_at` is set to the current time. Subnets further down keep their parents.

For example, with `10.0.0.0/8` containing `10.0.0.0/16`, which contains `10.0.1.0/24`, deleting `10.0.0.0/16` makes `10.0.0.0/8` the parent of `10.0.1.0/24`.

Addresses recorded in the deleted subnet move to its parent, and their `updated_at` is set to the current time. Addresses recorded in its child subnets stay with those children. The request is rejected with `409 Conflict`, and nothing is changed, in two cases:

- The subnet has recorded addresses and no parent to move them to. Delete its addresses first.
- A recorded address would become the parent's network or broadcast address, which IPv4 subnets of /30 or shorter reserve. For example, with `10.0.0.0/31` under `10.0.0.0/24` and the address `10.0.0.0` recorded in `10.0.0.0/31`, deleting `10.0.0.0/31` is rejected.

- `204 No Content` with no response body
- `404 Not Found` if no subnet has that ID
- `400 Bad Request` if `id` is not a valid UUID
- `409 Conflict` if the subnet's recorded addresses cannot move to a parent, as described above

Error messages returned in `error`:

| Condition | `error` |
| --- | --- |
| No subnet has that ID | `not found` |
| The subnet has recorded addresses and no parent | `subnet has recorded addresses and no parent subnet to move them to; delete its addresses first` |
| A recorded address would become the parent's network or broadcast address | `address 10.0.0.0 would become the network or broadcast address of the parent subnet 10.0.0.0/24; delete it first` |

## Addresses

### The address object

```json
{
  "id": "3f1c2b8e-9d4a-4e2f-8b6a-1c5d7e9f0a12",
  "address": "10.0.1.5",
  "subnet_id": "7a904281-76dd-4a32-a76c-f86f6bc0d839",
  "hostname": "web01",
  "description": "",
  "source": "manual",
  "created_at": "2026-09-30T12:00:00.123456Z",
  "updated_at": "2026-09-30T12:00:00.123456Z"
}
```

| Field | Type | Description |
| --- | --- | --- |
| `id` | string (UUID) | Unique identifier |
| `address` | string | A single IPv4 or IPv6 address, without a prefix length. Unique across all subnets. |
| `subnet_id` | string (UUID) | The most specific subnet that contains the address. Set by the server, and changed when subnets are created or deleted; see [`POST /api/subnets`](#post-apisubnets) and [`DELETE /api/subnets/{id}`](#delete-apisubnetsid). |
| `hostname` | string | Free text. Empty string when not set. |
| `description` | string | Free text. Empty string when not set. |
| `source` | string | How the address was recorded: `manual` for an address recorded with [`POST /api/addresses`](#post-apiaddresses), or `allocated` for an address handed out by [`POST /api/subnets/{id}/addresses/allocate`](#post-apisubnetsidaddressesallocate). The database also accepts `discovered`, reserved for network discovery, which is not built; no current endpoint sets it. |
| `created_at` | string | RFC 3339 timestamp, UTC |
| `updated_at` | string | RFC 3339 timestamp, UTC |

There is no endpoint to edit an address yet.

### `POST /api/addresses`

Role: editor. Records an address. The request body must be a JSON object sent with `Content-Type: application/json`.

```json
{
  "address": "10.0.1.5",
  "hostname": "web01",
  "description": "Rack 4 web server"
}
```

| Field | Type | Required | Rules |
| --- | --- | --- | --- |
| `address` | string | Yes | A single IPv4 or IPv6 address, without a prefix length: `10.0.1.5` is accepted, `10.0.1.5/32` is rejected with `422`. Must lie inside an existing subnet and must not already be recorded. |
| `hostname` | string | No | Leading and trailing whitespace is removed before validation and storage. After trimming, at most 253 characters. The format is not checked. Defaults to an empty string. |
| `description` | string | No | At most 1000 characters. Stored as sent, without trimming. Defaults to an empty string. |

Character limits count Unicode characters, not bytes. Fields not listed above are rejected with `422`, including `subnet_id` and `source`.

The server chooses the subnet: the address is placed in the most specific subnet that contains it, the one with the longest prefix. The new address has `source` set to `manual`.

An IPv4 subnet of /30 or shorter reserves its network and broadcast addresses, so they cannot be recorded in it. For example, `10.0.1.0` and `10.0.1.255` are rejected when they would be placed in `10.0.1.0/24`. IPv4 /31 and /32 subnets and IPv6 subnets reserve no addresses. The rule applies only to the subnet the address is placed in: `10.0.1.0` is accepted when the most specific subnet containing it is `10.0.0.0/16`.

Checks run in this order, and only the first failure is reported:

1. The field rules in the table, in the order `hostname`, `description`.
2. Some subnet must contain the address.
3. The address must not be the network or broadcast address of the subnet it is placed in.
4. The address must not already be recorded.

- `201 Created` with the new address object
- `400 Bad Request` if a field rule fails, no subnet contains the address, the address is reserved in its subnet, or the body is not valid JSON
- `409 Conflict` if the address is already recorded
- `415 Unsupported Media Type` if the `Content-Type` header is not `application/json`
- `422 Unprocessable Entity` if the body is valid JSON but does not match the request shape: `address` is missing or is not a valid address, a field has the wrong type, or an unknown field is present

Validation messages returned in `error`:

| Condition | `error` |
| --- | --- |
| `hostname` is too long | `hostname must be at most 253 characters` |
| `description` is too long | `description must be at most 1000 characters` |
| No subnet contains the address | `address 192.168.1.10 is not inside any subnet; create its subnet first` |
| The address is reserved in its subnet | `address 10.0.1.0 is the network or broadcast address of subnet 10.0.1.0/24` |
| The address is already recorded | `this address is already recorded` |

### `GET /api/addresses/{id}`

Role: viewer. Returns one address.

- `200 OK` with an address object
- `404 Not Found` if no address has that ID
- `400 Bad Request` if `id` is not a valid UUID

### `GET /api/subnets/{id}/addresses`

Role: viewer. Lists the addresses recorded in one subnet, in numeric order. Addresses in the subnet's child subnets are not included; list them through each child. There is no pagination or filtering yet.

- `200 OK` with an array of address objects, empty if the subnet has none
- `404 Not Found` if no subnet has that ID
- `400 Bad Request` if `id` is not a valid UUID

### `POST /api/subnets/{id}/addresses/allocate`

Role: editor. Records and returns the lowest free address in the subnet. The request body must be a JSON object sent with `Content-Type: application/json`. Send `{}` to allocate an address without details, or include a hostname and description:

```json
{
  "hostname": "web02",
  "description": "Rack 4 web server"
}
```

`hostname` and `description` follow the same rules as in [`POST /api/addresses`](#post-apiaddresses). Fields not listed are rejected with `422`.

The server searches the subnet's range from the lowest address up and returns the first address that is not excluded:

- In an IPv4 subnet of /30 or shorter, the network and broadcast addresses are excluded. In an IPv4 /31 or /32, every address is a candidate.
- In an IPv6 subnet shorter than /128, the first address of the subnet (the Subnet-Router anycast address) is excluded. The last address is a candidate. In an IPv6 /128, the single address is a candidate.
- Addresses inside the subnet's child subnets are excluded.
- Addresses already recorded in the subnet are excluded.

The new address has `source` set to `allocated`. For example, allocating from an empty `10.0.1.0/24` returns `10.0.1.1`; if `10.0.1.0/26` is a child subnet, it returns `10.0.1.64`. Allocating from an empty `fd00:10::/64` returns `fd00:10::1`.

Allocations take the same shared lock as creating and deleting subnets and recording addresses, so their placement decisions never interleave and concurrent requests never receive the same address.

Checks run in this order, and only the first failure is reported:

1. `id` must be a valid UUID.
2. The body must be JSON with the request shape.
3. The field rules, in the order `hostname`, `description`.
4. The subnet must exist.
5. The subnet must have a free address.

- `201 Created` with the new address object
- `400 Bad Request` if `id` is not a valid UUID, a field rule fails, or the body is not valid JSON
- `404 Not Found` if no subnet has that ID
- `409 Conflict` if the subnet has no free address
- `415 Unsupported Media Type` if the `Content-Type` header is not `application/json`
- `422 Unprocessable Entity` if the body is valid JSON but does not match the request shape: a field has the wrong type or an unknown field is present

Validation messages returned in `error`:

| Condition | `error` |
| --- | --- |
| `hostname` is too long | `hostname must be at most 253 characters` |
| `description` is too long | `description must be at most 1000 characters` |
| No subnet has that ID | `not found` |
| The subnet has no free address | `subnet 10.0.1.0/30 has no free addresses` |

### `DELETE /api/addresses/{id}`

Role: editor. Deletes one address.

- `204 No Content` with no response body
- `404 Not Found` if no address has that ID
- `400 Bad Request` if `id` is not a valid UUID

Error messages returned in `error`:

| Condition | `error` |
| --- | --- |
| No address has that ID | `not found` |

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
| `401` | `authentication required` | There are no valid credentials, or sign-in failed |
| `403` | `insufficient permissions` | The account's role does not allow this request |
| `403` | `this action requires signing in with a password, not an API token` | The request used an API token for an action that needs a password session |
| `404` | `not found` | The requested resource does not exist, or no endpoint matches a path under `/api/` |
| `409` | A description of the conflict | The request conflicts with existing data |
| `415` | A description of the problem | The request body is not declared as JSON |
| `422` | A description of the problem | The request body does not match the expected shape |
| `429` | `too many failed attempts; try again later` | Sign-in is rate limited; see `POST /api/session`. Carries a `Retry-After` header. |
| `500` | `internal server error` | An unexpected server or database error. Details are written to the server log, never returned to the client. |

For malformed request bodies and path parameters (the `400` body and path cases, `415`, and `422`), the status code and `error` text are produced by the Axum framework and may change when Axum is upgraded. Do not match on their exact wording.

## Audit log

Every sign-in attempt, sign-out, token creation and revocation, and every change to a subnet or address is recorded in the `audit_log` database table, in the same transaction as the change, so a change cannot be saved without its record. Each record holds the time, the account, the credential used (a session or token ID), the client address, the action, whether it succeeded, the affected object, and a summary of the change. The table has no API yet; read it with SQL. Database triggers reject updates, deletes, and truncation, so records cannot be altered through ordinary queries.
