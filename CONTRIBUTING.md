# Contributing to NetLedger

This document covers setting up a development environment and how changes are proposed, reviewed, and merged.

## Development setup

### Prerequisites

- Rust 1.94 or newer (the `rust-version` in `Cargo.toml`)
- PostgreSQL 14 or newer. Development uses PostgreSQL 18.
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

### Migrations

Create a reversible migration from the repository root:

```powershell
sqlx migrate add -r <name>
```

Every migration needs both an `.up.sql` and a `.down.sql` file. Never edit a migration that has been merged: databases record a checksum of every migration they apply and will refuse to start if an applied migration changes. Fix mistakes with a new migration instead.

### SQL queries and the offline cache

Queries written with `query!` and `query_as!` are checked against the database at compile time. CI has no database, so it builds from the query metadata committed in `.sqlx/`.

After adding or changing any checked query, or changing the schema a query depends on, regenerate the cache against a migrated database and commit it:

```powershell
cargo sqlx prepare --workspace -- --all-targets
```

The `-- --all-targets` flag includes queries in tests. Without it, CI fails to compile tests that use checked queries.

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

Run the same checks CI runs:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
cargo test --workspace --locked
```

Then confirm:

- If you changed a checked query or the schema, you regenerated and committed `.sqlx/`.
- `git status` does not list `.env` or any other file containing credentials.

### Documentation

Pull requests that change `crates/`, `web/`, `migrations/`, or the Compose file must also update `README.md`, `CONTRIBUTING.md`, or a file under `docs/`. The `docs-check` workflow enforces this. For changes with no user-facing effect, such as formatting or internal refactors, apply the `no-docs-needed` label instead.

Significant design decisions are recorded in [docs/adr/](docs/adr/).

## Reporting security issues

Do not report vulnerabilities in public issues. See [SECURITY.md](SECURITY.md).

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).
