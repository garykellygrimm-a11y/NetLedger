# Roadmap

This roadmap describes where NetLedger is headed and in what order. It is a plan, not a commitment: versions may be split, merged, or reordered as the project learns more. Released changes are recorded in [CHANGELOG.md](../CHANGELOG.md), and significant design decisions in [docs/adr/](adr/).

## Scope

NetLedger is an IP address management tool: a network source of truth for subnets and the addresses inside them. It records the intended state of a network, observes the actual state through discovery, and shows where the two disagree.

It is deliberately not a configuration management database (CMDB), and it does not run DHCP or DNS services. Instead, it aims to be a trustworthy data source for those systems: other tools can read from NetLedger through its API, and NetLedger can read leases and records from existing DHCP and DNS servers.

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

## Planned

### 0.6: Authentication, roles, and auditing

NetLedger cannot be deployed until it has this. Today, anyone who can reach the server can change anything.

- Sign-in for the web UI and API tokens for automation, such as provisioning pipelines that allocate addresses
- Roles, at minimum read-only, editor, and administrator
- An audit log of who changed what and when
- Design to cover local accounts, directory services, and smart card authentication, decided in an ADR before implementation
- Address status, so an address can be marked reserved as well as in use
- Address counts on the subnet list, so the web UI can show utilization for every subnet at once

### 0.7: Network discovery

Tier 1 discovery, which needs no agents and no credentials:

- Ping sweeps and reverse DNS lookups of selected subnets
- Addresses found by a scan are recorded with the `discovered` source and a last-seen time
- Designed to be approvable on controlled networks: off by default, enabled per subnet, rate-limited, and logged

### 0.8: Reconciliation

- Read-only import of DHCP leases and DNS records
- Comparison of recorded, discovered, and imported data, highlighting disagreements such as an active address nobody recorded or a recorded address that never answers

### 0.9: Design pass and packaging

- A polished, consistent web UI, including:
  - A per-subnet address heatmap showing assigned, reserved, free, and discovered-but-unrecorded addresses
  - Utilization bars in the subnet tree
  - A free-space view showing the largest available blocks in a subnet
  - A Ctrl+K command palette to jump to any address, subnet, or hostname
- Constraints for the design pass:
  - Styling is compiled at build time, so the strict Content Security Policy stays intact; no runtime CSS-in-JS
  - Fonts and icons are bundled into the binary, never loaded from a CDN
  - Status is never shown by color alone, to meet Section 508 accessibility requirements
- Installers and deployment guidance: an RPM for RHEL-family systems and a Windows service

### 1.0: Complete and tested

Version 1.0 is a quality bar rather than a feature list: the features above are complete, documented, covered by tests, and suitable for production use.

## API documentation

This work runs alongside the versions above.

- An OpenAPI description generated from the server code, so the reference documentation cannot drift from the implementation
- Interactive API documentation served by NetLedger itself, working fully offline
- A task-based cookbook, written after authentication so every example includes it, with each task shown in:
  - PowerShell 7 and Windows PowerShell 5.1, which handle HTTP errors differently
  - Python using only the standard library
  - curl and bash
  - Go
- Cookbook examples run in CI against a real server, so a broken example fails the build

## After 1.0

Ideas under consideration, in no particular order:

- Changing a subnet's CIDR
- Network topology from SNMP, LLDP, and CDP data
- Export for CMDB import jobs
- A Terraform provider for allocating and releasing addresses
- A NetLedger PowerShell module
- Credentialed inventory, only with a safe design for storing credentials
