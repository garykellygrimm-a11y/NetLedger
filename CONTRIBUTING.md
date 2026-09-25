# Contributing to NetLedger

This document covers setting up a development environment and how changes are proposed, reviewed, and merged.

## Development setup

### Prerequisites

- Rust 1.94 or newer (the `rust-version` in `Cargo.toml`)
- PostgreSQL 14 or newer. Development uses PostgreSQL 18.
- Node.js 24 LTS, or at least 22.12, needed only to work on the web front end in `web/`
- sqlx-cli, needed only to create migrations or change SQL queries:

```powershell
  cargo install sqlx-cli --no-default-features --features native-tls,postgres
```

### Database

**Native PostgreSQL (primary).** Install PostgreSQL and add its `bin` directory to your `PATH`. Then connect as the `postgres` superuser:

```powershell
psql -U postgres -h localhost
```

Create a login role and a database it owns. Use `\password` so the password never appears in command history or server logs:

```sql
CREATE ROLE netledger LOGIN;
\password netledger
CREATE DATABASE netledger OWNER netledger;
```

On a development machine, restrict PostgreSQL to local connections. Check with `SHOW listen_addresses;`. If it returns `*`, run the following and then restart the PostgreSQL service from an elevated shell:

```sql
ALTER SYSTEM SET listen_addresses = 'localhost';
```

**Docker Compose (alternative).** `compose.yml` starts PostgreSQL 18 bound to `127.0.0.1:5432`, with user, password, and database all set to `netledger`:

```powershell
docker compose up -d
```

### Configuration

Copy the template and adjust `DATABASE_URL` to match your database:

```powershell
Copy-Item .env.example .env
```

`.env` is ignored by Git. Never commit it. See [docs/configuration.md](docs/configuration.md) for every setting.

### Running

```powershell
cargo run -p netledger-server
```

The server applies any pending migrations on startup, so no separate migration step is needed.

### Web front end

The React and TypeScript front end in `web/` is built with Vite 8. Start the server first, then install dependencies once and start the Vite development server from `web/`:

```powershell
cd web
npm install
npm run dev
```

Vite serves the UI at `http://localhost:5173` and proxies requests under `/api` to the server at `http://127.0.0.1:8080`. The proxy target is set in `web/vite.config.ts` and does not follow `NETLEDGER_BIND_ADDR`, so if you run the server on a different address, change the proxy target locally.

Browser-side validation in the web UI's forms, such as required fields and length or range limits on inputs, is a convenience only. The API is the source of truth for validation: put validation rules in the API, not only in the front end, and do not rely on client-side checks to protect the data. The API's validation rules are listed in [docs/api.md](docs/api.md).

### Migrations

Create a reversible migration from the repository root:

```powershell
sqlx migrate add -r <name>
```

Every migration needs both an `.up.sql` and a `.down.sql` file. Never edit a migration that has been merged: databases record a checksum of every migration they apply and will refuse to start if an applied migration changes. Fix mistakes with a new migration instead.

### SQL queries and the offline cache

Queries written with `query!` and `query_as!` are checked against the database at compile time. CI builds with `SQLX_OFFLINE` set to `true`, so it compiles from the query metadata committed in `.sqlx/` instead of connecting to a database.

After adding or changing any checked query, or changing the schema a query depends on, regenerate the cache against a migrated database and commit it:

```powershell
cargo sqlx prepare --workspace -- --all-targets
```

The `-- --all-targets` flag includes queries in tests. Without it, CI fails to compile tests that use checked queries.

### Testing

Tests use `#[sqlx::test]`, which creates an isolated database for each test. For that, `DATABASE_URL` must point at a role that has the `CREATEDB` privilege.

For local development, grant the privilege by running the following as the `postgres` superuser:

```sql
ALTER ROLE netledger CREATEDB;
```

> **Warning:** Grant `CREATEDB` for development only. A production application role must never have `CREATEDB`.

Run the tests:

```powershell
cargo test --workspace
```

CI runs the tests against a PostgreSQL 18 service container.

## Workflow

All changes reach `main` through a pull request. Direct pushes to `main` are blocked, and every pull request must pass the required checks before merging.

1. Create a branch from an up-to-date `main`.
2. Make your changes in focused commits.
3. Run the local checks below.
4. Push the branch and open a pull request.
5. Review your own diff in the pull request before merging, even as the only maintainer.

### Branch names

Branches use the form `type/short-description`, with the same types as commit messages. Examples: `feat/subnet-read-api`, `fix/docs-agent-path`, `docs/restructure`.

### Commit messages

Commits follow [Conventional Commits](https://www.conventionalcommits.org/):

```text
type: short summary in the imperative mood
```

Common types are `feat`, `fix`, `docs`, `ci`, `build`, `chore`, `refactor`, `style`, and `test`. Put a space after the colon, keep the summary lowercase, and do not end it with a period. When the reason for a change is not obvious, add a body with a second `-m` flag.

Each commit has one purpose. Stage specific files rather than using `git add .`, and review `git diff --staged` before committing.

### Checks before opening a pull request

Run the same checks CI runs. The `ci` workflow has two jobs. The `rust` job runs:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
cargo test --workspace --locked
```

The `web` job uses Node.js 24 and runs the following from `web/`. `npm run build` also type-checks the front end:

```powershell
cd web
npm ci
npm run lint
npm run build
```

Then confirm:

- If you changed a checked query or the schema, you regenerated and committed `.sqlx/`.
- `git status` does not list `.env` or any other file containing credentials.

### Documentation

Pull requests that change `crates/`, `web/`, `migrations/`, or the Compose file must also update `README.md`, `CONTRIBUTING.md`, or a file under `docs/`. The `docs-check` workflow enforces this. Changes limited to `web/package.json` and `web/package-lock.json`, such as dependency updates, are exempt. For changes with no user-facing effect, such as formatting or internal refactors, apply the `no-docs-needed` label instead.

Significant design decisions are recorded in [docs/adr/](docs/adr/).

### Dependency updates

Dependabot, configured in `.github/dependabot.yml`, checks weekly for updates to Cargo dependencies, npm dependencies in `web/`, and GitHub Actions. It waits 7 days after a release before proposing an update. Minor and patch updates to Cargo dependencies are grouped into one pull request, and minor and patch updates to npm dependencies into another. GitHub Actions updates are grouped into one pull request. Major TypeScript upgrades are ignored until the Vite template adopts them.

## Releases

Releases are automated with [Knope](https://knope.tech/), based on the Conventional Commits described in [Commit messages](#commit-messages). Knope is configured in `knope.toml`. `feat` and `fix` commits trigger a release; other commit types, such as `docs`, `ci`, and `chore`, do not. Changeset files, described below, can also trigger one.

On every push to `main`, the `prepare-release` workflow runs Knope. If there are releasable changes since the last release, Knope bumps the version in `Cargo.toml` and `Cargo.lock`, updates `CHANGELOG.md`, force-pushes the result to the `chore/release` branch, and opens or updates a pull request from that branch to `main` titled `chore: release <version>`. If there is nothing to release, the workflow exits without opening a pull request. A maintainer can also run the workflow manually and set `override_version` to force a specific version.

Merging the release pull request runs the `release` workflow. It tags the release as `v<version>`, creates a GitHub release that uses the version's `CHANGELOG.md` section as its notes, and uploads statically linked `netledger-server` builds for x64 Linux (musl) and x64 Windows:

- `netledger-server-linux-x64.tar.gz`
- `netledger-server-windows-x64.zip`

Each archive has a matching `.sha256` file containing its SHA-256 checksum.

When a change needs a release note or a version bump that its commit messages do not express, create a changeset with Knope and commit the file it writes to `.changeset/` along with the change:

```powershell
knope document-change
```

## Reporting security issues

Do not report vulnerabilities in public issues. See [SECURITY.md](SECURITY.md).

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).
