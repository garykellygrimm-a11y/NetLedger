# NetLedger

NetLedger is an IP address management (IPAM) tool for tracking subnets and address allocations. It is designed to run anywhere from a single Windows workstation to an enterprise or air-gapped datacenter, from one codebase configured through environment variables. The backend is a Rust web server built on Axum, backed by PostgreSQL through SQLx. A React and TypeScript front end is planned.

## Status

NetLedger is in early development and is not ready for production use. The server applies its own database schema on startup, exposes health checks, and provides endpoints to list, view, create, and delete subnets. There is no authentication yet.

## Quick start

Requires Rust 1.94 or newer and a PostgreSQL database. See [CONTRIBUTING.md](CONTRIBUTING.md#development-setup) for full setup, including creating the database.

```powershell
Copy-Item .env.example .env
cargo run -p netledger-server
curl.exe http://127.0.0.1:8080/health/ready
```

## Documentation

- [CONTRIBUTING.md](CONTRIBUTING.md): development setup, workflow, and pull request requirements
- [docs/configuration.md](docs/configuration.md): environment variables
- [docs/api.md](docs/api.md): HTTP endpoints and error format
- [docs/adr/](docs/adr/): architecture decision records
- [SECURITY.md](SECURITY.md): reporting vulnerabilities

## Project layout

- `crates/netledger-server`: the Axum HTTP server
- `migrations/`: SQL schema migrations, embedded in the server binary and applied on startup
- `.sqlx/`: committed query metadata so the project builds without a database
- `compose.yml`: optional local PostgreSQL container for development
- `docs/`: reference documentation and decision records

## Roadmap

### Built

- Server with embedded migrations, liveness and readiness checks
- Subnet API: `GET /subnets`, `GET /subnets/{id}`, `POST /subnets` with validation, and `DELETE /subnets/{id}`
- JSON error responses
- Automated API tests
- CI with formatting, linting, build, and test checks; Dependabot; CodeQL

### Planned

- Individual IP address tracking and network discovery
- Authentication
- React and TypeScript front end
- Installers and deployment guidance for Windows and RHEL

## License

NetLedger is released under the [MIT License](LICENSE).
