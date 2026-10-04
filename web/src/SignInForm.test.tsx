import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { SubnetTable } from './SubnetTable.tsx'
import type { Subnet } from './types.ts'

const office: Subnet = {
  id: '7a904281-76dd-4a32-a76c-f86f6bc0d839',
  cidr: '10.0.1.0/24',
  name: 'Office',
  description: '',
  vlan_id: 100,
  parent_id: null,
  created_at: '2026-09-22T02:41:36Z',
  updated_at: '2026-09-22T02:41:36Z',
}

function renderTable(canEdit: boolean) {
  render(
    <SubnetTable
      subnets={[office]}
      deletingId={null}
      selectedId={null}
      canEdit={canEdit}
      onSelect={vi.fn()}
      onDelete={vi.fn()}
      onUpdated={vi.fn()}
    />,
  )
}

describe('SubnetTable', () => {
  it('shows edit and delete controls to editors', () => {
    renderTable(true)

    expect(screen.getByRole('button', { name: 'Edit 10.0.1.0/24' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Delete 10.0.1.0/24' })).toBeInTheDocument()
  })

  it('hides edit and delete controls from viewers but keeps the subnet selectable', () => {
    renderTable(false)

    expect(screen.queryByRole('button', { name: 'Edit 10.0.1.0/24' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Delete 10.0.1.0/24' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Show addresses in 10.0.1.0/24' })).toBeInTheDocument()
    expect(screen.getAllByRole('columnheader')).toHaveLength(4)
  })
})
