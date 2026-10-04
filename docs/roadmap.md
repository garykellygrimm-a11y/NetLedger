# Roadmap

This roadmap describes where NetLedger is headed and in what order. It is a plan, not a commitment: versions may be split, merged, or reordered as the project learns more. Released changes are recorded in [CHANGELOG.md](../CHANGELOG.md), and significant design decisions in [docs/adr/](adr/).

## Scope

NetLedger is an IP address management tool: a network source of truth for subnets and the addresses inside them. It records the intended state of a network, observes the actual state through discovery, and shows where the two disagree.

It is deliberately not a configuration management database (CMDB), and it does not run DHCP or DNS services. Instead, it aims to be a trustworthy data source for those systems: other tools can read from NetLedger through its API, and NetLedger can read leases and records from existing DHCP and DNS servers.

PostgreSQL is the only supported database. NetLedger relies on PostgreSQL's network address types, exclusion constraints, and transactional guarantees to keep its data correct, so other databases are out of scope.

## Built

| Version | Highlights |
| --- | --- |
| 0.1.0 | Subnet API with validation, consistent JSON errors, embedded migrations, health checks, and automated tests |
| 0.2.0 | API moved under `/api`; web UI to list, create, and delete subnets, embedded in a single release binary with security headers |
| 0.3.0 | Subnet containment and overlap rules enforced by the database, with specific error messages |
| 0.3.1 | Concurrent conflicting creates return `409 Conflict` instead of `500` |
| 0.4.0 | Automatic subnet placement, so subnets can be created in any order; editing a subnet's name, description, and VLAN with `PATCH`; deleting a subnet moves its children to its parent |
| 0.4.1 | Web UI shows subnets as an indented tree and edits them in place |
| 0.5.0 | IP address tracking: addresses are placed in their most specific subnet, move with the subnet tree, and can be allocated as the next free address |
| 0.5.1 | Web UI for addresses: list, record, allocate, and delete, with subnet utilization |

## Criteria for 1.0

Version 1.0 is a promise of stability: after it, breaking changes require a new major version. It ships when all of these are true:

1. **Deployable in controlled environments**, including smart card sign-in through an F5 load balancer.
2. **Secure enough for review**: authentication, roles, auditing, HTTPS, accurate FIPS claims, and signed releases.
3. **Useful day to day as an IPAM tool**, including network discovery.
4. **The intended user interface**, after a dedicated design pass.
5. **Installable and documented**, including a documented, STIG-aligned way to deploy PostgreSQL and API examples in several languages.
6. **Hardened**, through a release phase that adds nothing new and verifies everything.

Work that does not serve these criteria is planned after 1.0.

## Planned before 1.0

Each version below ships as a series of patch releases as its pieces land.

### 0.6: Authentication core

NetLedger could not be deployed until it had this: before 0.6.0, anyone who could reach the server could change anything. See [ADR 0004](adr/0004-authentication-core.md).

Shipped in 0.6.0:

- User accounts, each with one or more sign-in identities, so new sign-in methods attach to existing accounts
- Local password sign-in following NIST SP 800-63B Revision 4, with rate-limited failed attempts
- Configurable password hashing: Argon2id by default, or PBKDF2 with HMAC-SHA-256 where FIPS 140 is required
- Server-side browser sessions that end on sign-out, after 12 hours, or after 30 minutes idle
- Roles: viewer, editor, and administrator, checked on every API route
- An audit log of who changed what, when, from where, and with what outcome
- No default credentials; the first administrator is created with the `create-admin` command

Planned as 0.6 patch releases:

- 0.6.1: API tokens for automation, such as provisioning pipelines that allocate addresses
- 0.6.2: Address status, so an address can be marked reserved as well as in use, and address counts on the subnet list, so the web UI can show utilization for every subnet at once
- 0.6.3: Built-in HTTPS
- 0.6.4: Account management in the web UI for administrators (create, disable, reset passwords, end sessions, read the audit log), and a first-run setup page that creates the first administrator in the browser using a one-time token printed by the server, as an alternative to `create-admin`

### 0.7: Smart card sign-in through an F5

- CAC and PIV sign-in where an F5 BIG-IP validates the certificate and forwards it to NetLedger
- Trust in forwarded certificates is off by default and limited to configured proxy addresses
- Deployment guidance for the F5 configuration and for restricting direct access to NetLedger

### 0.8: Network discovery

Tier 1 discovery, which needs no agents and no credentials:

- Ping sweeps and reverse DNS lookups of selected subnets
- Addresses found by a scan are recorded with the `discovered` source and a last-seen time
- Designed to be approvable on controlled networks: off by default, enabled per subnet, rate-limited, and logged

### 0.9: Design pass

- A polished, consistent web UI, including:
  - A per-subnet address heatmap showing assigned, reserved, free, and discovered-but-unrecorded addresses
  - Utilization bars in the subnet tree
  - A free-space view showing the largest available blocks in a subnet
  - A Ctrl+K command palette to jump to any address, subnet, or hostname
- Constraints for the design pass:
  - Styling is compiled at build time, so the strict Content Security Policy stays intact; no runtime CSS-in-JS
  - Fonts and icons are bundled into the binary, never loaded from a CDN
  - Status is never shown by color alone, to meet Section 508 accessibility requirements

### 0.10: Packaging, signing, and documentation

- An RPM for RHEL-family systems and a Windows service
- A `setup` command that walks through database connection, bind address, trusted proxies, migrations, and the first administrator, with every prompt also available as a flag so installers and configuration management can run it unattended
- Signed releases: Authenticode-signed Windows binaries and installer, GPG-signed RPMs, and documented verification steps. IdenTrust is the likely certificate vendor, pending confirmation of which certificates target environments accept. Releases are designed so organizations can verify and re-sign them with their own certificates.
- A software bill of materials (SBOM) and build provenance for each release
- A documented, STIG-aligned way to deploy PostgreSQL for NetLedger
- The API documentation described below

### 0.11: Hardening

No new features. Bugs found become 0.11 patch releases, and 1.0.0 ships after a round of testing finds nothing new.

- A security review of the code against the architecture decision records
- Clean installs and upgrades with existing data, on RHEL-family systems and Windows Server
- A walkthrough of every documented instruction and example
- Vulnerability scanning of the kind used in controlled environments

## API documentation

This work runs alongside the versions above and is complete by 0.10.

- An OpenAPI description generated from the server code, so the reference documentation cannot drift from the implementation
- Interactive API documentation served by NetLedger itself, working fully offline
- A task-based cookbook, written after authentication so every example includes it, with each task shown in:
  - PowerShell 7 and Windows PowerShell 5.1, which handle HTTP errors differently
  - Python using only the standard library
  - curl and bash
  - Go
- Cookbook examples run in CI against a real server, so a broken example fails the build

## After 1.0

New functionality after 1.0 ships as minor versions (1.1, 1.2, and so on). Ideas under consideration, in no particular order:

- Directory sign-in through OIDC and LDAP
- Smart card sign-in directly to NetLedger, without an F5
- Reconciliation: read-only import of DHCP leases and DNS records, compared against recorded and discovered data
- A FIPS build that uses a FIPS 140-validated cryptographic module for TLS
- Changing a subnet's CIDR
- Network topology from SNMP, LLDP, and CDP data
- Export for CMDB import jobs
- A Terraform provider for allocating and releasing addresses
- A NetLedger PowerShell module
- Credentialed inventory, only with a safe design for storing credentials
