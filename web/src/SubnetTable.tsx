import { useState } from 'react'
import { EditSubnetRow } from './EditSubnetRow.tsx'
import { toTreeRows } from './tree.ts'
import type { Subnet } from './types.ts'

type Props = {
  subnets: Subnet[]
  deletingId: string | null
  selectedId: string | null
  canEdit: boolean
  onSelect: (subnet: Subnet) => void
  onDelete: (subnet: Subnet) => void
  onUpdated: () => void
}

export function SubnetTable({
  subnets,
  deletingId,
  selectedId,
  onSelect,
  canEdit,
  onDelete,
  onUpdated,
}: Props) {
  const [editingId, setEditingId] = useState<string | null>(null)

  if (subnets.length === 0) {
    return <p className="empty">No subnets yet.</p>
  }

  const rows = toTreeRows(subnets)
  const busy = editingId !== null || deletingId !== null

  return (
    <table>
      <thead>
        <tr>
          <th scope="col">Network</th>
          <th scope="col">Name</th>
          <th scope="col">VLAN</th>
          <th scope="col">Description</th>
          {canEdit && (
            <th scope="col">
              <span className="visually-hidden">Actions</span>
            </th>
          )}
        </tr>
      </thead>
      <tbody>
        {rows.map(({ subnet, parent, depth }) =>
          subnet.id === editingId ? (
            <EditSubnetRow
              key={subnet.id}
              subnet={subnet}
              depth={depth}
              onCancel={() => setEditingId(null)}
              onSaved={() => {
                setEditingId(null)
                onUpdated()
              }}
            />
          ) : (
            <tr key={subnet.id} className={subnet.id === selectedId ? 'selected' : undefined}>
              <td className="mono cidr-cell" style={{ paddingLeft: `${0.75 + depth * 1.5}rem` }}>
                {depth > 0 && (
                  <span className="tree-marker" aria-hidden="true">
                    └{' '}
                  </span>
                )}
                <button
                  type="button"
                  className="link mono"
                  onClick={() => onSelect(subnet)}
                  aria-pressed={subnet.id === selectedId}
                  aria-label={`Show addresses in ${subnet.cidr}`}
                >
                  {subnet.cidr}
                </button>
                {parent && <span className="visually-hidden"> (inside {parent.cidr})</span>}
              </td>
              <td>{subnet.name}</td>
              <td>{subnet.vlan_id ?? '—'}</td>
              <td>{subnet.description}</td>
              {canEdit && (
                <td className="actions">
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => setEditingId(subnet.id)}
                    disabled={busy}
                    aria-label={`Edit ${subnet.cidr}`}
                  >
                    Edit
                  </button>
                  <button
                    type="button"
                    className="danger"
                    onClick={() => onDelete(subnet)}
                    disabled={busy}
                    aria-label={`Delete ${subnet.cidr}`}
                  >
                    {deletingId === subnet.id ? 'Deleting…' : 'Delete'}
                  </button>
                </td>
              )}
            </tr>
          ),
        )}
      </tbody>
    </table>
  )
}
