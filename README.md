# NetLedger

[![CI](https://github.com/garykellygrimm-a11y/NetLedger/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/garykellygrimm-a11y/NetLedger/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/garykellygrimm-a11y/NetLedger)](https://github.com/garykellygrimm-a11y/NetLedger/releases/latest)
[![Minimum Rust version](https://img.shields.io/badge/dynamic/toml?url=https%3A%2F%2Fraw.githubusercontent.com%2Fgarykellygrimm-a11y%2FNetLedger%2Fmain%2FCargo.toml&query=%24.workspace.package%5B%27rust-version%27%5D&label=rust&suffix=%2B)](CONTRIBUTING.md#prerequisites)
[![License: MIT](https://img.shields.io/github/license/garykellygrimm-a11y/NetLedger)](LICENSE)

NetLedger is an IP address management (IPAM) tool for tracking subnets and address allocations. It is designed to run anywhere from a single Windows workstation to an enterprise or air-gapped datacenter, from one codebase configured through environment variables. The backend is a Rust web server built on Axum, backed by PostgreSQL through SQLx. The front end, in early development, uses React and TypeScript.

## Status

NetLedger is in early development and is not ready for production use. The server applies its own database schema on startup, exposes health checks, and provides endpoints to list, view, create, edit, and delete subnets. Editing through the API covers a subnet's name, description, and VLAN; its CIDR cannot be changed yet. The server places each subnet under the smallest subnet that contains it, so subnets can be created in any order. The web UI lists subnets as an indented tree that follows the server's placement, with each subnet shown under its parent, and screen readers announce which subnet each one sits inside. The web UI also has a form to create subnets. When the API rejects a subnet, the form shows the API's error message. After a subnet is created, the list refreshes to show it. Each row in the subnet list has an Edit button that edits the subnet's name, VLAN, and description in place. Enter saves and Escape cancels, and only the changed fields are sent to the API with `PATCH`. The network (CIDR) cannot be edited in the web UI. Each row also has a Delete button that asks for confirmation before deleting the subnet. Deleting a subnet moves its child subnets up to its parent. The API also records individual IP addresses. Each address is placed in the most specific subnet that contains it and moves when subnets are created or deleted. The API can allocate the next free address in a subnet. Addresses cannot be edited yet. In the web UI, selecting a subnet's network opens a panel that lists the addresses recorded directly in that subnet, with their hostname, description, and source. For an IPv4 subnet with no child subnets, the panel also shows how many of its usable addresses are recorded. The panel has a form to record an address or allocate the next free address in the subnet, with an optional hostname and description, and each address has a Delete button that asks for confirmation. The server places a recorded address in the most specific subnet that contains it, which may not be the selected subnet. When the API rejects a delete, such as deleting a top-level subnet that still has addresses, the web UI shows the API's error message. Every API request needs a signed-in account. The first administrator is created with the `create-admin` command; there are no default credentials. Accounts have a role: viewers can read, and editors can also make changes. Browsers sign in with a password, and sessions end on sign-out, after 12 hours, or after 30 minutes without a request. Scripts use API tokens, which each account creates and revokes from the user menu in the web UI. Failed sign-ins are rate limited, and every sign-in attempt and every change is recorded in an audit log.

The server serves both the API and the web UI at its bind address. Release builds embed the built web UI in the binary, so a release is a single file.

## Quick start

A release needs a PostgreSQL database with the `btree_gist` extension files installed. See [CONTRIBUTING.md](CONTRIBUTING.md#database) for creating one. If you are upgrading an existing database, see [Upgrading an existing database](CONTRIBUTING.md#upgrading-an-existing-database) first.

1. Download the archive for your platform from the repository's GitHub releases, `netledger-server-windows-x64.zip` or `netledger-server-linux-x64.tar.gz`, and extract it. Releases after 0.1.0 include the web UI.
2. Set `DATABASE_URL` in the environment or in a `.env` file in the working directory. See [docs/configuration.md](docs/configuration.md).
3. Run the server:

   ```powershell
   .\netledger-server.exe
   ```

4. Open `http://127.0.0.1:8080` in a browser. This is the default bind address.

To build and run from source instead, with Rust 1.94 or newer and Node.js, see [CONTRIBUTING.md](CONTRIBUTING.md#development-setup):

```powershell
Copy-Item .env.example .env
npm --prefix web install
npm --prefix web run build
cargo run -p netledger-server
```

## Documentation

- [CONTRIBUTING.md](CONTRIBUTING.md): development setup, workflow, and pull request requirements
- [docs/configuration.md](docs/configuration.md): environment variables
- [docs/api.md](docs/api.md): HTTP endpoints, error format, web UI routing, and security headers
- [docs/adr/](docs/adr/): architecture decision records
- [docs/roadmap.md](docs/roadmap.md): planned work through 1.0 and beyond
- [SECURITY.md](SECURITY.md): reporting vulnerabilities
- [CHANGELOG.md](CHANGELOG.md): changes in each release

## Project layout

- `crates/netledger-server`: the Axum HTTP server
- `web/`: the React and TypeScript front end, built with Vite into `web/dist`, which the server serves
- `migrations/`: SQL schema migrations, embedded in the server binary and applied on startup
- `.sqlx/`: committed query metadata so the project builds without a database
- `compose.yml`: optional local PostgreSQL container for development
- `docs/`: reference documentation and decision records

## Roadmap

Address status, built-in HTTPS, account management, network discovery, and a design pass are next. See [docs/roadmap.md](docs/roadmap.md) for the full plan through 1.0 and beyond.

## License

NetLedger is released under the [MIT License](LICENSE).
