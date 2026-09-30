import { toTreeRows } from './tree.ts'
import type { Subnet } from './types.ts'

type Props = {
  subnets: Subnet[]
  deletingId: string | null
  onDelete: (subnet: Subnet) => void
}

export function SubnetTable({ subnets, deletingId, onDelete }: Props) {
  if (subnets.length === 0) {
    return <p className="empty">No subnets yet.</p>
  }

  const rows = toTreeRows(subnets)

  return (
    <table>
      <thead>
        <tr>
          <th scope="col">Network</th>
          <th scope="col">Name</th>
          <th scope="col">VLAN</th>
          <th scope="col">Description</th>
          <th scope="col">
            <span className="visually-hidden">Actions</span>
          </th>
        </tr>
      </thead>
      <tbody>
        {rows.map(({ subnet, parent, depth }) => (
          <tr key={subnet.id}>
            <td className="mono cidr-cell" style={{ paddingLeft: `${0.75 + depth * 1.5}rem` }}>
              {depth > 0 && (
                <span className="tree-marker" aria-hidden="true">
                  └{' '}
                </span>
              )}
              {subnet.cidr}
              {parent && <span className="visually-hidden"> (inside {parent.cidr})</span>}
            </td>
            <td>{subnet.name}</td>
            <td>{subnet.vlan_id ?? '—'}</td>
            <td>{subnet.description}</td>
            <td className="actions">
              <button
                type="button"
                className="danger"
                onClick={() => onDelete(subnet)}
                disabled={deletingId !== null}
                aria-label={`Delete ${subnet.cidr}`}
              >
                {deletingId === subnet.id ? 'Deleting…' : 'Delete'}
              </button>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}
