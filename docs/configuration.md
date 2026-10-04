# Configuration

NetLedger is configured entirely through environment variables. On startup, the server loads a `.env` file from the working directory if one exists. Variables already set in the environment take precedence over values in `.env`, so on a real server the service configuration always wins.

## Runtime variables

| Variable | Required | Default | Description |
| --- | --- | --- | --- |
| `DATABASE_URL` | Yes | None | PostgreSQL connection URL |
| `NETLEDGER_BIND_ADDR` | No | `127.0.0.1:8080` | Address and port the HTTP server listens on |
| `NETLEDGER_PASSWORD_HASH` | No | `argon2id` | Algorithm for hashing account passwords: `argon2id` or `pbkdf2-sha256` |
| `RUST_LOG` | No | `netledger_server=info` | Log filter |

### `DATABASE_URL`

A PostgreSQL connection URL:

```text
postgres://USER:PASSWORD@HOST:PORT/DATABASE
```

If it is not set, the server exits at startup with `DATABASE_URL must be set`. If the database cannot be reached, the server exits with `failed to connect to the database`.

Characters in the password with special meaning in URLs must be percent-encoded: `@` as `%40`, `:` as `%3A`, `/` as `%2F`, `#` as `%23`, `?` as `%3F`, and `%` as `%25`.

**TLS.** Database connections support TLS through rustls, trusting certificates from the operating system's certificate store. Enable it with the `sslmode` parameter:

```text
postgres://netledger:PASSWORD@db.example.internal:5432/netledger?sslmode=verify-full
```

`verify-full` encrypts the connection and verifies the server's certificate and hostname. Use it for any database that is not on the same machine.

### `NETLEDGER_BIND_ADDR`

An IP address and port, such as `127.0.0.1:8080` or `0.0.0.0:8080`. The server serves both the API and the web UI at this address. The default listens only on the loopback interface, so the server is unreachable from other machines. An invalid value stops the server at startup with `NETLEDGER_BIND_ADDR must be an address and port, like 127.0.0.1:8080`.

NetLedger has no authentication yet, and its API can create, change, and delete subnets and addresses (see [api.md](api.md)). Do not bind it to a non-loopback address on an untrusted network.

### `NETLEDGER_PASSWORD_HASH`

The algorithm used to hash account passwords. Either `argon2id` (the default) or `pbkdf2-sha256`. Any other value stops the server at startup with `NETLEDGER_PASSWORD_HASH must be argon2id or pbkdf2-sha256`.

Argon2id is the stronger general-purpose choice and is recommended unless policy requires otherwise. PBKDF2 with HMAC-SHA-256 is provided for environments that require FIPS 140-approved algorithms. Choosing it makes password hashing use an approved algorithm; it does not make every cryptographic operation in NetLedger FIPS-validated.

Each stored hash records its own algorithm and parameters, so changing this setting does not invalidate existing passwords. A password hashed with the previous algorithm is rehashed with the configured one the next time its owner signs in successfully.

### `RUST_LOG`

A `tracing-subscriber` filter directive. For example, `netledger_server=debug` enables debug logging. If unset or invalid, the default is `netledger_server=info`.

In PowerShell, set it for a single session:

```powershell
$env:RUST_LOG = "netledger_server=debug"
cargo run -p netledger-server
```

## Build-time variables

| Variable | Description |
| --- | --- |
| `SQLX_OFFLINE` | When `true`, checked queries compile against the committed `.sqlx/` metadata instead of connecting to `DATABASE_URL`. CI sets this. |

## Commands

The server binary accepts an optional command. With no command, it starts the HTTP server. Every command reads the same environment variables, connects to the database, and applies pending migrations before running.

### `create-admin <username>`

Creates an account with the administrator role and a local password:

```powershell
netledger-server create-admin gary
```

The password is prompted for twice without being displayed; it is never accepted as a command-line argument. Usernames are stored in lowercase and may contain only letters, digits, `.`, `_`, and `-`. Passwords must be 15 to 256 characters and must not appear on the built-in list of common passwords or contain the username.

NetLedger ships with no default credentials, so this command is how the first administrator is created. It can also be run later to regain access if every administrator is locked out. Running it requires the same access as running the server. Each use writes an audit log entry recording the operating system user and host that ran it.
