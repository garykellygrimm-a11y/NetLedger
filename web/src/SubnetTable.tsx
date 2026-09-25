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
        {subnets.map((subnet) => (
          <tr key={subnet.id}>
            <td className="mono">{subnet.cidr}</td>
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
