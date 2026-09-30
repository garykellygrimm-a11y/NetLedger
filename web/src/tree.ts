import type { Subnet } from './types.ts'

export type TreeRow = {
  subnet: Subnet
  parent: Subnet | null
  depth: number
}

export function toTreeRows(subnets: Subnet[]): TreeRow[] {
  const byId = new Map(subnets.map((subnet) => [subnet.id, subnet]))
  const children = new Map<string | null, Subnet[]>()

  for (const subnet of subnets) {
    const parentId =
      subnet.parent_id !== null && byId.has(subnet.parent_id) ? subnet.parent_id : null
    const siblings = children.get(parentId)
    if (siblings) {
      siblings.push(subnet)
    } else {
      children.set(parentId, [subnet])
    }
  }

  const rows: TreeRow[] = []

  const visit = (parent: Subnet | null, depth: number) => {
    for (const subnet of children.get(parent?.id ?? null) ?? []) {
      rows.push({ subnet, parent, depth })
      visit(subnet, depth + 1)
    }
  }

  visit(null, 0)
  return rows
}
