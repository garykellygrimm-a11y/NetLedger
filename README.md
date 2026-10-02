# NetLedger

NetLedger is an IP address management (IPAM) tool for tracking subnets and address allocations. It is designed to run anywhere from a single Windows workstation to an enterprise or air-gapped datacenter, from one codebase configured through environment variables. The backend is a Rust web server built on Axum, backed by PostgreSQL through SQLx. The front end, in early development, uses React and TypeScript.

## Status

NetLedger is in early development and is not ready for production use. The server applies its own database schema on startup, exposes health checks, and provides endpoints to list, view, create, edit, and delete subnets. Editing through the API covers a subnet's name, description, and VLAN; its CIDR cannot be changed yet. The server places each subnet under the smallest subnet that contains it, so subnets can be created in any order. The web UI lists subnets as an indented tree that follows the server's placement, with each subnet shown under its parent, and screen readers announce which subnet each one sits inside. The web UI also has a form to create subnets. When the API rejects a subnet, the form shows the API's error message. After a subnet is created, the list refreshes to show it. Each row in the subnet list has an Edit button that edits the subnet's name, VLAN, and description in place. Enter saves and Escape cancels, and only the changed fields are sent to the API with `PATCH`. The network (CIDR) cannot be edited in the web UI. Each row also has a Delete button that asks for confirmation before deleting the subnet. Deleting a subnet moves its child subnets up to its parent. The API also records individual IP addresses. Each address is placed in the most specific subnet that contains it and moves when subnets are created or deleted. The API can allocate the next free address in a subnet. Addresses cannot be edited yet, and the web UI does not show or manage addresses. There is no authentication yet.

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

### Built

- Server with embedded migrations, liveness and readiness checks
- Subnet API: `GET /api/subnets`, `GET /api/subnets/{id}`, `POST /api/subnets` with validation, `PATCH /api/subnets/{id}` for name, description, and VLAN, and `DELETE /api/subnets/{id}`
- Subnet containment and overlap rules, enforced by the database
- Automatic subnet placement: each new subnet goes under the smallest subnet that contains it, and existing subnets inside it move under it, so subnets can be created in any order; deleting a subnet moves its children to its parent
- Address API: `POST /api/addresses`, `GET /api/addresses/{id}`, `GET /api/subnets/{id}/addresses`, and `DELETE /api/addresses/{id}`
- Automatic address placement: each address goes in the most specific subnet that contains it, and addresses move when subnets are created or deleted
- Next free address allocation: `POST /api/subnets/{id}/addresses/allocate`
- JSON error responses
- Web UI served by the server and embedded in release builds, with security headers on every response
- Web UI: subnet list shown as an indented tree
- Create subnets from the web UI
- Edit a subnet's name, VLAN, and description in place from the web UI
- Delete subnets from the web UI, with confirmation
- Automated API tests
- CI with formatting, linting, build, and test checks for the server and linting and build checks for the web UI; Dependabot; CodeQL

### Planned

- Web UI for addresses
- Network discovery
- Authentication
- Changing a subnet's CIDR through the API and the web UI
- Installers and deployment guidance for Windows and RHEL

## License

NetLedger is released under the [MIT License](LICENSE).
