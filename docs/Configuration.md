# Configuration

NetLedger is configured entirely through environment variables. On startup, the server loads a `.env` file from the working directory if one exists. Variables already set in the environment take precedence over values in `.env`, so on a real server the service configuration always wins.

## Runtime variables

| Variable | Required | Default | Description |
| --- | --- | --- | --- |
| `DATABASE_URL` | Yes | None | PostgreSQL connection URL |
| `NETLEDGER_BIND_ADDR` | No | `127.0.0.1:8080` | Address and port the HTTP server listens on |
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

An IP address and port, such as `127.0.0.1:8080` or `0.0.0.0:8080`. The default listens only on the loopback interface, so the server is unreachable from other machines. An invalid value stops the server at startup with `NETLEDGER_BIND_ADDR must be an address and port, like 127.0.0.1:8080`.

NetLedger has no authentication yet. Do not bind it to a non-loopback address on an untrusted network.

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
