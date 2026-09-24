import type { Subnet } from './types.ts'

type Props = {
  subnets: Subnet[]
}

export function SubnetTable({ subnets }: Props) {
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
        </tr>
      </thead>
      <tbody>
        {subnets.map((subnet) => (
          <tr key={subnet.id}>
            <td className="mono">{subnet.cidr}</td>
            <td>{subnet.name}</td>
            <td>{subnet.vlan_id ?? '—'}</td>
            <td>{subnet.description}</td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}
