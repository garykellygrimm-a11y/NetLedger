# 0003. Store each address as its own row in its most specific subnet

- Status: Accepted
- Date: 2026-10-01

## Context

NetLedger needed to track individual IP addresses, not just subnets. Subnets nest, so one address can lie inside several of them, for example `10.0.0.0/16`, `10.0.1.0/24`, and `10.0.1.0/26`. The design had to settle which subnet owns an address, what happens to addresses when subnets are created or deleted above or around them, and how to hand out free addresses without two clients receiving the same one.

Subnet placement already worked this way: the server, not the client, chooses a subnet's parent, and the database enforces containment and overlap (see `migrations/20260925133937_enforce_subnet_hierarchy.up.sql`).

## Decision

Each address is one row in the `address` table. Its `subnet_id` points to the most specific subnet that contains it, the server chooses that subnet, and every operation that makes a placement decision takes one shared database lock.

In detail:

- **One row per host.** `address` is an `inet` with a full-length mask (`/32` or `/128`), enforced by the `address_is_single_host` check. It is `UNIQUE` across the whole table, not per subnet, because an IP address is recorded once no matter how the subnets around it are arranged.
- **Most specific subnet.** Clients send only the address. The server places it in the smallest subnet that contains it, and the `address_within_subnet` trigger rejects any row whose address lies outside its subnet.
- **Addresses move with the subnet tree.** Creating a subnet moves the parent's addresses that fall inside it into the new subnet. Deleting a subnet moves its addresses to its parent. If a move would leave an address as the network or broadcast address of its new IPv4 subnet (larger than /31), or a deleted top-level subnet has addresses with nowhere to go, the request returns `409 Conflict` and nothing changes.
- **`ON DELETE RESTRICT`.** The foreign key from `address.subnet_id` to `subnet` refuses to delete a subnet that still owns addresses. The server moves addresses before it deletes the subnet, so the restriction is a safety net: no code path can silently delete addresses.
- **One shared lock.** Creating and deleting subnets, recording addresses, and allocating addresses each take the same transaction-scoped advisory lock (`pg_advisory_xact_lock` with `PLACEMENT_LOCK_KEY`) before they read the tree. Deleting an address and editing a subnet's name, description, or VLAN make no placement decisions and rely on ordinary row-level locking.
- **Allocation in the server.** `POST /api/subnets/{id}/addresses/allocate` reads the subnet's child subnets and recorded addresses under the lock, then walks upward from the first usable address. It skips child subnets whole, the IPv4 network and broadcast addresses, and the IPv6 subnet-router anycast address (the first address).
- **`source` records where an address came from:** `manual`, `allocated`, or `discovered`. `discovered` is reserved for network discovery, which is not built yet.

## Alternatives considered

- **Derive the owning subnet at query time instead of storing `subnet_id`.** This removes the need to move addresses when subnets change, but every listing would need a containment search across the whole tree, and the database could not enforce "this address belongs to exactly one subnet" or block deleting a subnet that holds addresses.
- **Let the client choose the subnet.** This contradicts how subnets are placed and invites inconsistent data, such as an address recorded in `10.0.0.0/16` while `10.0.1.0/24` exists beneath it.
- **Cascade deletes from subnet to address.** Deleting a subnet would silently delete every address recorded in it. `RESTRICT` plus moving addresses to the parent keeps data unless someone deletes it on purpose.
- **Per-subnet locks or optimistic retries.** A lock per subnet does not cover operations that change the tree itself, such as creating a subnet that takes addresses from its parent while another request records an address in that parent. A single lock is simpler to reason about, and placement writes are rare enough that serializing them costs nothing noticeable.
- **Allocate with a set-returning SQL query.** Generating candidate addresses in SQL is awkward for large IPv6 subnets and harder to test. The walk in Rust jumps over child subnets in one step, and the lock already makes it safe.

## Consequences

- Listing a subnet's addresses is a single indexed query on `address_subnet_id_idx`, and it returns only that subnet's own addresses, not those of its children.
- Every endpoint that changes the subnet tree must also move addresses and must take the shared lock. New placement operations, such as network discovery or moving a subnet, must do the same.
- Deleting a top-level subnet that has addresses now fails with `409 Conflict`. Clients, including the web UI, need to surface that error.
- Throughput for placement writes is limited to one at a time. If that ever becomes a bottleneck, the lock is the place to revisit.
- An address can be recorded at a position that later becomes a reserved address, for example `10.0.1.0` under `10.0.0.0/16`. Creating `10.0.1.0/24` afterwards then returns `409 Conflict` until the address is deleted.
