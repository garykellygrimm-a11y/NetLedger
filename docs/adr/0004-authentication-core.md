# 0004. Authenticate with accounts that can hold several identities, starting with local passwords

- Status: Accepted
- Date: 2026-10-04

## Context

NetLedger has no authentication. Anyone who can reach the server can create, change, and delete subnets and addresses, so it cannot be deployed anywhere that matters.

Its intended environments shape the requirements:

- Controlled networks, including Department of Defense networks, where smart card (CAC and PIV) sign-in is required, applications are often behind an F5 BIG-IP, and FIPS 140-validated cryptography may be mandatory.
- Organizations with directory services, through both LDAP and OIDC.
- Small and air-gapped installations with no directory at all.
- Automation, such as provisioning pipelines that allocate and release addresses, which cannot sign in through a web page.

Supporting every sign-in method at once would delay a usable release. The design therefore has to start small without blocking the methods that come later.

## Decision

NetLedger authenticates **accounts**, each of which can hold one or more **identities** (ways of proving who you are). The first release supports local passwords and API tokens, with roles and an audit log; smart cards and directory services are added later as new identity types on the same accounts.

The decisions that follow from that:

- **Passwords** follow NIST SP 800-63B Revision 4: at least 15 characters, at least 64 allowed, no composition rules, screening against a blocklist of common and compromised passwords, no periodic expiration, and rate-limited failed attempts.
- **Password hashing is configurable.** Argon2id is the default. PBKDF2 with HMAC-SHA-256 is available where FIPS 140-approved algorithms are required. Each stored hash records its algorithm and parameters, so both kinds can coexist, and a password is rehashed with the configured algorithm at its next successful sign-in.
- **Browser sessions are stored on the server.** The browser holds only a random session identifier in a cookie that is `HttpOnly`, `Secure`, and `SameSite=Strict`. Signing out, disabling an account, or removing an identity ends the affected sessions immediately.
- **API tokens** are random, shown once at creation, stored only as hashes, carry a recognizable prefix so secret scanners can detect leaked tokens, belong to an account and act with its role, may expire, can be revoked, and record when they were last used.
- **Roles** are viewer, editor, and administrator, enforced by the server on every request.
- **Every change is audited** in the same transaction as the change itself, so no change can be recorded without its audit entry or the reverse.
- **HTTPS is built in**, because `Secure` cookies require it, and it is the foundation for smart card sign-in. Running behind a reverse proxy that terminates TLS remains supported.
- **There are no default credentials.** The first administrator is created by an explicit setup command.

## Alternatives considered

- **JSON Web Tokens for browser sessions.** Rejected because a JWT stays valid until it expires, so removing someone's access immediately requires extra machinery that server-side sessions provide for free. Immediate revocation matters more here than avoiding a database lookup per request.
- **One sign-in method per account**, such as a username column with a password hash. Rejected because adding smart cards or a directory later would mean migrating every account, and because one person often needs two methods: a smart card for daily use and a local password for break-glass access.
- **Smart card sign-in first.** Rejected because it depends on accounts, roles, sessions, and auditing, which must exist first, and because it can only be tested in environments with the right infrastructure. The account model is designed for it from the start, so adding it later requires no migration.
- **Composition rules and periodic password changes.** Rejected because NIST SP 800-63B Revision 4 prohibits composition rules and periodic changes, and because they push people toward predictable passwords.
- **A single fixed hashing algorithm.** Rejected because no single choice fits every environment: Argon2id is the stronger general recommendation, but it is not FIPS-approved, and some environments require approved algorithms.
- **Relying only on a reverse proxy for authentication.** Rejected as the only option because small installations have no proxy, and because NetLedger needs its own accounts and roles for auditing regardless. Proxy-provided identities are planned as one identity type among several.

## Consequences

- Every endpoint must check a role. A new endpoint without an authorization check is a security bug, so tests must cover authorization for every route.
- Every database change must write an audit entry in the same transaction. Code that changes data outside that path is a defect.
- Sessions add a database lookup to every browser request. This is acceptable for NetLedger's expected load.
- Deployments need a TLS certificate, either for NetLedger itself or for a proxy in front of it.
- Some sign-in attempts fail by design: rate limiting can slow down a legitimate user who mistypes repeatedly.
- Claims about FIPS compliance must be precise. Choosing PBKDF2 makes password hashing use an approved algorithm, but it does not make every cryptographic path in NetLedger FIPS-validated; documentation must not say otherwise.
- Adding smart cards, LDAP, or OIDC means adding an identity type, not redesigning accounts. Each still needs its own design, including how a certificate or directory account is matched to a NetLedger account.
