# NetLedger

NetLedger is an IP address management (IPAM) tool for public and private organizations. The backend is a Rust web server built on Axum, backed by a Postgres database via `sqlx`. A React and TypeScript front end is planned but does not exist yet. The project is at an early stage: the server connects to Postgres, applies its own database schema on startup, and exposes liveness and readiness health checks, but no IPAM data endpoints exist yet.

## Project layout

The repository is a Cargo workspace (`Cargo.toml`, resolver 3, members `crates/*`).

- `crates/netledger-server`: Axum HTTP server binary. Entry point is `crates/netledger-server/src/main.rs`; environment configuration is loaded in `crates/netledger-server/src/config.rs`.
- `migrations/`: SQL migration files (up/down pairs) for the Postgres database, embedded into the server binary and applied automatically on startup. Currently contains one migration, `20260922024136_create_subnet`, which creates the `subnet` table.
- `compose.yml`: Docker Compose file that starts a local Postgres 18 instance for development, as an alternative to a native install.
- `.env.example`: template for the environment variables the server reads (`DATABASE_URL`, `NETLEDGER_BIND_ADDR`).
- Frontend: no frontend directory exists yet.

## Prerequisites

- Rust 1.94 or newer. This is the `rust-version` set in `[workspace.package]` in `Cargo.toml`, and the workspace uses Rust edition 2024. `netledger-server` inherits it, so Cargo enforces it.
- A reachable Postgres 18 database. Either a native install or the `compose.yml` Docker Compose setup works; see "Local Postgres database" below. This is required to run the server — `DATABASE_URL` must be set and reachable, or the server fails to start.
- `sqlx-cli`, only if you need to author new migrations (see below). Not required to build, run, or test the server, since the server applies existing migrations itself.

There are no prerequisites for the frontend because none exists.

## Build, run, and test

Commands are run from the repository root.

```sh
cargo build --workspace --locked
cargo run -p netledger-server
cargo test --workspace --locked
```

CI (`.github/workflows/ci.yml`) also runs formatting and lint checks:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

`cargo build` and `cargo test` do not require a database connection: `sqlx::migrate!` embeds the SQL files from `migrations/` at compile time by reading them from disk, and the workspace does not use `sqlx::query!`-style macros that need a live database or `DATABASE_URL` at compile time. `cargo run -p netledger-server` does require a running, reachable Postgres database, because the server connects and runs migrations before it starts serving requests.

The repository currently contains no automated tests, so `cargo test` compiles the workspace and runs nothing. There is no frontend to build or test.

### Environment variables

Read in `crates/netledger-server/src/config.rs`:

- `DATABASE_URL` (required): Postgres connection string. `Config::from_env()` returns an error and the server does not start if this is unset.
- `NETLEDGER_BIND_ADDR` (optional): address and port the HTTP server listens on, e.g. `127.0.0.1:8080`. Defaults to `127.0.0.1:8080` if unset. If set, it must parse as a valid socket address (`host:port`); an invalid value, including an empty string, causes the server to fail at startup with the error "NETLEDGER_BIND_ADDR must be an address and port, like 127.0.0.1:8080".

Read in `main.rs` directly:

- `RUST_LOG` (optional): log filter using the `tracing-subscriber` `EnvFilter` syntax. If unset or invalid, the filter defaults to `netledger_server=info`.

`main.rs` also calls `dotenvy::dotenv()` on startup, so a `.env` file in the working directory (if present) is loaded into the process environment before configuration is read.

```sh
RUST_LOG=netledger_server=debug cargo run -p netledger-server
```

### Local Postgres database

The server requires `DATABASE_URL` to point at a reachable Postgres database, and it applies the SQL migrations in `migrations/` itself on every startup via `sqlx::migrate!` — there is no separate manual migration step. Two ways to provide that database:

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

This starts a Postgres 18 container (`compose.yml`) with user, password, and database all set to `netledger`, bound to `127.0.0.1:5432`.

**Either way:**

```sh
cp .env.example .env
```

`.env.example` sets `DATABASE_URL=postgres://netledger:netledger@localhost:5432/netledger` and `NETLEDGER_BIND_ADDR=127.0.0.1:8080`. Adjust the `DATABASE_URL` host, port, credentials, and database name in `.env` to match whichever setup you actually used — the role and database you create natively may not match the Compose defaults. `.env` is listed in `.gitignore` and is not committed; the default credentials in `.env.example` are for local development only.

### Creating new migrations

Applying existing migrations does not require any extra tooling; the server does that itself on startup. Creating new migration files does require `sqlx-cli`:

```sh
cargo install sqlx-cli --no-default-features --features native-tls,postgres
sqlx migrate add <name>
```

This install command is untested against this repository.

## API endpoints

No endpoint currently requires authentication.

- `GET /health`: liveness check. Returns HTTP 200 with the plain-text body `ok`. Does not touch the database.
- `GET /health/ready`: readiness check. Runs `SELECT 1` against the database pool; returns HTTP 200 with the body `ready` if it succeeds, or HTTP 503 with the body `database unavailable` if the query fails.

```sh
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/health/ready
```

## Security and deployment notes

The default bind address is the loopback interface (`127.0.0.1:8080`), so the server is not reachable from other hosts unless `NETLEDGER_BIND_ADDR` is explicitly set to a non-loopback address. The code has no authentication or TLS support. Any deployment beyond local use would need code changes that do not exist yet.

The database connection pool is capped at 10 connections with a 5-second acquire timeout (`PgPoolOptions` in `main.rs`); there is no other connection tuning or retry logic.

The `compose.yml` Postgres service binds only to `127.0.0.1:5432` and uses fixed development credentials; it is not intended for production use. `.env` (containing `DATABASE_URL`) is git-ignored so local credentials are not committed.

## Roadmap

Built:

- Cargo workspace with the `netledger-server` crate.
- Axum server that connects to Postgres via `sqlx` (`PgPoolOptions`) and applies embedded SQL migrations (`sqlx::migrate!("../../migrations")`) on every startup.
- `GET /health` (liveness, no database access) and `GET /health/ready` (readiness, checks the database with `SELECT 1`); neither requires authentication.
- Configuration via `DATABASE_URL` (required) and `NETLEDGER_BIND_ADDR` (optional, defaults to `127.0.0.1:8080`), read in `crates/netledger-server/src/config.rs`.
- Logging through `tracing` with an environment-based filter (`RUST_LOG`).
- CI workflow (`.github/workflows/ci.yml`) running `cargo fmt`, `cargo clippy`, `cargo build`, and `cargo test`.
- A `docs-check` workflow (`.github/workflows/docs-check.yml`) that fails pull requests touching `crates/`, `web/`, `migrations/`, or `compose.yml`/`compose.yaml` unless `README.md` or `docs/` are also updated, or the PR is labeled `no-docs-needed`.
- Local Postgres development setup: native install instructions and `compose.yml` as an alternative, plus `.env.example`.
- A SQL migration creating the `subnet` table (`migrations/20260922024136_create_subnet`), applied automatically by the server.

Planned, not yet implemented:

- IPAM API endpoints (addresses, subnets, and related resources) beyond the health checks.
- Authentication.
- TLS.
- React and TypeScript front end.

## License and contribution

NetLedger is released under the MIT license. See [LICENSE](LICENSE). There are no formal contribution guidelines yet, but the `docs-check` CI workflow requires pull requests that change `crates/`, `web/`, `migrations/`, or the Compose file to also update `README.md` or `docs/`, unless labeled `no-docs-needed`.
