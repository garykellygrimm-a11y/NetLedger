# NetLedger

NetLedger is an IP address management (IPAM) tool for public and private organizations. The backend is a Rust web server built on Axum. A React and TypeScript front end is planned but does not exist yet. The project is at an early stage: the only implemented functionality is a health check route.

## Project layout

The repository is a Cargo workspace (`Cargo.toml`, resolver 3, members `crates/*`).

- `crates/netledger-server`: Axum HTTP server binary. Entry point is `crates/netledger-server/src/main.rs`.
- Frontend: no frontend directory exists yet.

## Prerequisites

Rust 1.85 or newer. This is the `rust-version` set in `[workspace.package]` in `Cargo.toml`, and the workspace uses Rust edition 2024. `netledger-server` inherits it, so Cargo enforces it.

There are no prerequisites for the frontend because none exists.

## Build, run, and test

Commands are run from the repository root.

```sh
cargo build
cargo run -p netledger-server
cargo test
```

The server binds to `127.0.0.1:8080`. The address is hard-coded in `main.rs` and cannot be changed by configuration.

No environment variables are required. The optional `RUST_LOG` variable sets the log filter, using the `tracing-subscriber` `EnvFilter` syntax. If it is unset or invalid, the filter defaults to `netledger_server=info`.

```sh
RUST_LOG=netledger_server=debug cargo run -p netledger-server
```

The repository currently contains no tests, so `cargo test` compiles the workspace and runs nothing. There is no frontend to build or test.

## API endpoints

Only one endpoint is implemented. It requires no authentication.

- `GET /health`: returns HTTP 200 with the plain-text body `ok`.

```sh
curl http://127.0.0.1:8080/health
```

## Security and deployment notes

The server listens only on the loopback interface (`127.0.0.1`), so it is not reachable from other hosts as configured. The code has no authentication, TLS, database, or configuration file support. Any deployment beyond local use would need code changes that do not exist yet.

## Roadmap

Built:

- Cargo workspace with the `netledger-server` crate.
- Axum server with `GET /health`.
- Logging through `tracing` with an environment-based filter.

Planned, not yet implemented:

- IPAM API endpoints (addresses, subnets, and related resources).
- Authentication.
- Configurable listen address.
- React and TypeScript front end.

## License and contribution

NetLedger is released under the MIT license. See [LICENSE](LICENSE). There are no contribution guidelines yet.
