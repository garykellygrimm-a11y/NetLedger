---
default: minor
---

# deleting a subnet moves its addresses to its parent

`DELETE /api/subnets/{id}` now moves the subnet's recorded addresses to its parent. It returns `409 Conflict` instead of deleting the subnet when the subnet has addresses but no parent, or when a moved address would become the parent's network or broadcast address. Delete or move those addresses first. Creating a subnet likewise moves the parent's addresses that fall inside it, and returns `409 Conflict` if one of them would become the new subnet's network or broadcast address.
