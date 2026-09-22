# NetLedger

NetLedger is an IP address management (IPAM) tool for public and private organizations. The backend is a Rust web server built on Axum. A React and TypeScript front end is planned but does not exist yet. The project is at an early stage: the only implemented HTTP functionality is a health check route, and a Postgres schema for subnets has been added but is not yet wired into the server code.

## Project layout

The repository is a Cargo workspace (`Cargo.toml`, resolver 3, members `crates/*`).

- `crates/netledger-server`: Axum HTTP server binary. Entry point is `crates/netledger-server/src/main.rs`.
- `migrations/`: SQL migration files (up/down pairs) for the Postgres database. Currently contains one migration that creates the `subnet` table.
- `compose.yml`: Docker Compose file that starts a local Postgres 18 instance for development.
- `.env.example`: template for the `DATABASE_URL` used to connect to the local Postgres instance.
- Frontend: no frontend directory exists yet.

## Prerequisites

- Rust 1.85 or newer. This is the `rust-version` set in `[workspace.package]` in `Cargo.toml`, and the workspace uses Rust edition 2024. `netledger-server` inherits it, so Cargo enforces it.
- Docker (or another Compose-compatible runtime) if you want to run the local Postgres database defined in `compose.yml`. This is only needed for database work; the current server code does not connect to a database.

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

The server binds to `127.0.0.1:8080`. The address is hard-coded in `main.rs` and cannot be changed by configuration.

The optional `RUST_LOG` variable sets the log filter, using the `tracing-subscriber` `EnvFilter` syntax. If it is unset or invalid, the filter defaults to `netledger_server=info`.

```sh
RUST_LOG=netledger_server=debug cargo run -p netledger-server
```

The repository currently contains no automated tests, so `cargo test` compiles the workspace and runs nothing. There is no frontend to build or test.

### Local Postgres database

A `compose.yml` at the repository root starts a Postgres 18 container with a `netledger` user, password, and database, matching `.env.example`:

```sh
cp .env.example .env
docker compose up -d
```

`.env` is listed in `.gitignore` and is not committed. The default credentials in `.env.example` (`netledger`/`netledger`) are for local development only.

The `migrations/` directory contains a SQL migration (`20260922024136_create_subnet`) that creates the `subnet` table. No migration-runner tool or database driver is wired into the codebase yet (`sqlx` and similar crates are not workspace dependencies, and no code reads `DATABASE_URL`), so migrations must currently be applied by hand, for example with `psql`:

```sh
psql "$DATABASE_URL" -f migrations/20260922024136_create_subnet.up.sql
```

This command is untested against this repository; it is a generic `psql` invocation, not a tool configured in the project.

## API endpoints

Only one endpoint is implemented. It requires no authentication.

- `GET /health`: returns HTTP 200 with the plain-text body `ok`.

```sh
curl http://127.0.0.1:8080/health
```

## Security and deployment notes

The server listens only on the loopback interface (`127.0.0.1`), so it is not reachable from other hosts as configured. The code has no authentication, TLS, database connectivity, or configuration file support. Any deployment beyond local use would need code changes that do not exist yet.

The `compose.yml` Postgres service also binds only to `127.0.0.1:5432` and uses fixed development credentials; it is not intended for production use. `.env` (containing `DATABASE_URL`) is git-ignored so local credentials are not committed.

## Roadmap

Built:

- Cargo workspace with the `netledger-server` crate.
- Axum server with `GET /health` (no authentication required).
- Logging through `tracing` with an environment-based filter.
- CI workflow (`.github/workflows/ci.yml`) running `cargo fmt`, `cargo clippy`, `cargo build`, and `cargo test`.
- A `docs-check` workflow (`.github/workflows/docs-check.yml`) that fails pull requests touching `crates/` or `web/` unless `README.md` or `docs/` are also updated, or the PR is labeled `no-docs-needed`.
- Local Postgres development setup (`compose.yml`, `.env.example`).
- A SQL migration creating the `subnet` table (`migrations/`), not yet used by any application code.

Planned, not yet implemented:

- Database connectivity in the server (no driver/ORM crate such as `sqlx` is a dependency yet, and no code reads `DATABASE_URL`).
- IPAM API endpoints (addresses, subnets, and related resources).
- Authentication.
- Configurable listen address.
- React and TypeScript front end.

## License and contribution

NetLedger is released under the MIT license. See [LICENSE](LICENSE). There are no formal contribution guidelines yet, but the `docs-check` CI workflow requires pull requests that change `crates/` or `web/` to also update `README.md` or `docs/`, unless labeled `no-docs-needed`.
