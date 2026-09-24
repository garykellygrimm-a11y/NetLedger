# NetLedger

NetLedger is an IP address management (IPAM) tool for public and private organizations. The backend is a Rust web server built on Axum, backed by a Postgres database via `sqlx`. A React and TypeScript front end is planned but does not exist yet. The project is at an early stage: the server connects to Postgres, applies its own database schema on startup, exposes liveness and readiness health checks, and provides read-only endpoints to list subnets and fetch a single subnet. There are no endpoints yet for creating, updating, or deleting data.

## Project layout

The repository is a Cargo workspace (`Cargo.toml`, resolver 3, members `crates/*`).

- `crates/netledger-server`: Axum HTTP server binary. `src/main.rs` is the entry point and defines the health routes, `src/config.rs` reads environment configuration, `src/subnets.rs` holds the subnet endpoints, and `src/error.rs` maps errors to JSON responses. `build.rs` makes Cargo rebuild the crate when files in `migrations/` change.
- `migrations/`: SQL migration files (up/down pairs) for the Postgres database, embedded into the server binary and applied automatically on startup. Currently contains one migration, `20260922024136_create_subnet`, which creates the `subnet` table.
- `.sqlx/`: committed `sqlx` offline query metadata for the compile-time-checked queries in `netledger-server`. CI builds against it with `SQLX_OFFLINE=true`.
- `compose.yml`: Docker Compose file that starts a local Postgres 18 instance for development, as an alternative to a native install.
- `.env.example`: template for the environment variables the server reads (`DATABASE_URL`, `NETLEDGER_BIND_ADDR`).
- `.gitattributes`: forces LF line endings for `*.sql` files, so migration files have the same contents on every platform.
- Frontend: no frontend directory exists yet.

## Prerequisites

- Rust 1.94 or newer. This is the `rust-version` set in `[workspace.package]` in `Cargo.toml`, and the workspace uses Rust edition 2024. `netledger-server` inherits it, so Cargo enforces it.
- A reachable Postgres 18 database. Either a native install or the `compose.yml` Docker Compose setup works; see "Local Postgres database" below. This is required to run the server: `DATABASE_URL` must be set and reachable, or the server fails to start.
- `sqlx-cli`, only if you author new migrations or change SQL queries (see "Changing SQL queries" and "Creating new migrations" below). It is not required to build, run, or test the server.

There are no prerequisites for the frontend because none exists.

## Build, run, and test

Commands are run from the repository root. CI (`.github/workflows/ci.yml`) runs these steps, in this order, with `SQLX_OFFLINE=true` set for the whole job:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
cargo test --workspace --locked
```

To run the server:

```sh
cargo run -p netledger-server
```

`cargo run -p netledger-server` requires a running, reachable Postgres database, because the server connects and runs migrations before it starts serving requests.

The repository currently contains no automated tests, so `cargo test` compiles the workspace and runs nothing. There is no frontend to build or test.

### Compile-time query checking and `SQLX_OFFLINE`

The subnet endpoints use `sqlx::query_as!`, which checks SQL against a database schema at compile time. The metadata for those checks is committed in `.sqlx/`, and CI sets `SQLX_OFFLINE=true` so it builds from that metadata without a database.

To build locally without a database, do the same:

```sh
SQLX_OFFLINE=true cargo build --workspace --locked
```

```powershell
$env:SQLX_OFFLINE = "true"; cargo build --workspace --locked
```

Without `SQLX_OFFLINE=true`, and with `DATABASE_URL` set (for example from a local `.env`), the `sqlx` macros check the queries against that live database instead. That database must be reachable and already have the migrations applied (running the server once applies them).

`sqlx::migrate!` embeds the SQL files from `migrations/` at compile time by reading them from disk; that part never needs a database.

### Changing SQL queries

If you add or change a `sqlx::query!`/`query_as!` call, or change the schema those queries depend on, regenerate `.sqlx/` against a migrated database and commit the result. Otherwise the CI build, which runs offline, will fail. With `sqlx-cli` installed and `DATABASE_URL` pointing at a migrated database:

```sh
cargo sqlx prepare --workspace
```

This command is not referenced anywhere else in the repository and is untested against it.

### Environment variables

Read in `crates/netledger-server/src/config.rs`:

- `DATABASE_URL` (required): Postgres connection string. `Config::from_env()` returns the error "DATABASE_URL must be set" and the server does not start if this is unset.
- `NETLEDGER_BIND_ADDR` (optional): address and port the HTTP server listens on, e.g. `127.0.0.1:8080`. Defaults to `127.0.0.1:8080` if unset. If set, it must parse as a valid socket address (`host:port`); an invalid value, including an empty string, causes the server to fail at startup with the error "NETLEDGER_BIND_ADDR must be an address and port, like 127.0.0.1:8080".

Read in `main.rs` directly:

- `RUST_LOG` (optional): log filter using the `tracing-subscriber` `EnvFilter` syntax. If unset or invalid, the filter defaults to `netledger_server=info`.

`main.rs` also calls `dotenvy::dotenv()` on startup, so a `.env` file in the working directory (if present) is loaded into the process environment before configuration is read.

```sh
RUST_LOG=netledger_server=debug cargo run -p netledger-server
```

Used at build time only:

- `SQLX_OFFLINE`: when `true`, the `sqlx` query macros use the committed `.sqlx/` metadata instead of connecting to `DATABASE_URL`. Set by CI; see above.

### Local Postgres database

The server requires `DATABASE_URL` to point at a reachable Postgres database, and it applies the SQL migrations in `migrations/` itself on every startup via `sqlx::migrate!`. There is no separate manual migration step. Two ways to provide that database:

**1. Native PostgreSQL install (Windows)**

Install PostgreSQL 18 locally, then create a login role and database for NetLedger. For example, using `psql` as the `postgres` superuser:

```sql
CREATE ROLE netledger LOGIN PASSWORD 'netledger';
CREATE DATABASE netledger OWNER netledger;
```

Adjust the password to match whatever you put in `.env`. This talks to Postgres running directly on the host, not a container. This setup and the exact `psql` commands are untested against this repository.

**2. Docker Compose (alternative)**

```sh
docker compose up -d
```

This starts a Postgres 18 container (`compose.yml`) with user, password, and database all set to `netledger`, bound to `127.0.0.1:5432`, with data stored in the `netledger-pg` named volume.

**Either way:**

```sh
cp .env.example .env
```

`.env.example` sets `DATABASE_URL=postgres://netledger:netledger@localhost:5432/netledger` and `NETLEDGER_BIND_ADDR=127.0.0.1:8080`. Adjust the `DATABASE_URL` host, port, credentials, and database name in `.env` to match whichever setup you actually used; the role and database you create natively may not match the Compose defaults. `.env` is listed in `.gitignore` and is not committed; the default credentials in `.env.example` are for local development only.

### Creating new migrations

Applying existing migrations does not require any extra tooling; the server does that itself on startup. Creating new migration files does require `sqlx-cli`:

```sh
cargo install sqlx-cli --no-default-features --features native-tls,postgres
sqlx migrate add -r <name>
```

`-r` creates a reversible up/down pair, matching the existing migration. If the new migration changes tables used by existing queries, regenerate `.sqlx/` as described in "Changing SQL queries". These commands are untested against this repository.

## API endpoints

No endpoint currently requires authentication.

- `GET /health`: liveness check. Returns HTTP 200 with the plain-text body `ok`. Does not touch the database.
- `GET /health/ready`: readiness check. Runs `SELECT 1` against the database pool; returns HTTP 200 with the body `ready` if it succeeds, or HTTP 503 with the body `database unavailable` if the query fails.
- `GET /subnets`: lists all subnets as a JSON array, ordered by `cidr`. There is no pagination or filtering.
- `GET /subnets/{id}`: returns one subnet as a JSON object. `id` must be a UUID. Returns HTTP 404 if no subnet has that id.

Both subnet endpoints return objects with these fields, taken from the `Subnet` struct in `crates/netledger-server/src/subnets.rs`:

| Field | JSON type | Notes |
| --- | --- | --- |
| `id` | string | UUID |
| `cidr` | string | Network in CIDR notation, e.g. `10.0.0.0/24`; unique |
| `name` | string | |
| `description` | string | Defaults to an empty string in the database |
| `vlan_id` | number or null | Constrained to 1 through 4094 by the database |
| `parent_id` | string or null | UUID of a parent subnet |
| `created_at` | string | Timestamp (UTC) |
| `updated_at` | string | Timestamp (UTC) |

Errors from the subnet endpoints are JSON objects with a single `error` field: `{"error":"not found"}` with HTTP 404, or `{"error":"internal server error"}` with HTTP 500 for database errors (the underlying error is logged, not returned). An `id` that is not a valid UUID is rejected by Axum's path extractor before the query runs, with an Axum-generated plain-text error rather than this JSON body.

There is no API for creating subnets, so `GET /subnets` returns an empty array until rows are inserted into the `subnet` table by other means.

```sh
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/health/ready
curl http://127.0.0.1:8080/subnets
curl http://127.0.0.1:8080/subnets/<uuid>
```

## Security and deployment notes

The default bind address is the loopback interface (`127.0.0.1:8080`), so the server is not reachable from other hosts unless `NETLEDGER_BIND_ADDR` is explicitly set to a non-loopback address. The code has no authentication or TLS support for the HTTP server, and the subnet endpoints return all stored subnet data to any caller. Any deployment beyond local use would need code changes that do not exist yet.

Database errors on the subnet endpoints are logged server-side and returned to clients only as a generic `internal server error` message.

The database connection pool is capped at 10 connections with a 5-second acquire timeout (`PgPoolOptions` in `main.rs`); there is no other connection tuning or retry logic. `sqlx` is built with the `tls-rustls-ring-native-roots` feature, but the server code does not configure database TLS itself; any TLS settings come from the `DATABASE_URL` connection string.

The `compose.yml` Postgres service binds only to `127.0.0.1:5432` and uses fixed development credentials; it is not intended for production use. `.env` (containing `DATABASE_URL`) is git-ignored so local credentials are not committed.

Because the CI build uses `SQLX_OFFLINE=true`, a server binary can be built on a machine with no database access, using the committed `.sqlx/` metadata.

## Roadmap

Built:

- Cargo workspace with the `netledger-server` crate.
- Axum server that connects to Postgres via `sqlx` (`PgPoolOptions`) and applies embedded SQL migrations (`sqlx::migrate!("../../migrations")`) on every startup.
- `GET /health` (liveness, no database access) and `GET /health/ready` (readiness, checks the database with `SELECT 1`).
- Read-only subnet endpoints: `GET /subnets` and `GET /subnets/{id}`, using compile-time-checked `sqlx::query_as!` queries with committed `.sqlx/` offline metadata.
- A SQL migration creating the `subnet` table (`migrations/20260922024136_create_subnet`), applied automatically by the server.
- Configuration via `DATABASE_URL` (required) and `NETLEDGER_BIND_ADDR` (optional, defaults to `127.0.0.1:8080`), read in `crates/netledger-server/src/config.rs`, with `.env` loading through `dotenvy`.
- Logging through `tracing` with an environment-based filter (`RUST_LOG`).
- CI workflow (`.github/workflows/ci.yml`) running `cargo fmt`, `cargo clippy`, `cargo build`, and `cargo test` with `SQLX_OFFLINE=true`.
- A `docs-check` workflow (`.github/workflows/docs-check.yml`) that fails pull requests touching `crates/`, `web/`, `migrations/`, or `compose.yml`/`compose.yaml` unless `README.md` or `docs/` are also updated, or the PR is labeled `no-docs-needed`.
- Dependabot configuration (`.github/dependabot.yml`) for weekly Cargo and GitHub Actions updates.
- Local Postgres development setup: native install instructions and `compose.yml` as an alternative, plus `.env.example`.

Planned, not yet implemented:

- Endpoints to create, update, and delete subnets.
- IPAM resources beyond subnets, such as individual IP addresses.
- Authentication.
- TLS.
- React and TypeScript front end.

## License and contribution

NetLedger is released under the MIT license. See [LICENSE](LICENSE). There are no formal contribution guidelines yet. Two repository rules affect pull requests:

- The CI build runs with `SQLX_OFFLINE=true`, so changes to SQL queries or the schema they use must include regenerated `.sqlx/` metadata.
- The `docs-check` CI workflow requires pull requests that change `crates/`, `web/`, `migrations/`, or the Compose file to also update `README.md` or `docs/`, unless labeled `no-docs-needed`.
