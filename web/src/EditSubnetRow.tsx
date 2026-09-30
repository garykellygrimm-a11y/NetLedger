import { useState } from 'react'
import type { KeyboardEvent } from 'react'
import { ApiError, updateSubnet } from './api.ts'
import type { Subnet, UpdateSubnetInput } from './types.ts'

type Props = {
  subnet: Subnet
  depth: number
  onCancel: () => void
  onSaved: () => void
}

type SaveState =
  | { status: 'idle' }
  | { status: 'saving' }
  | { status: 'error'; message: string }

export function EditSubnetRow({ subnet, depth, onCancel, onSaved }: Props) {
  const [name, setName] = useState(subnet.name)
  const [vlan, setVlan] = useState(subnet.vlan_id === null ? '' : String(subnet.vlan_id))
  const [description, setDescription] = useState(subnet.description)
  const [save, setSave] = useState<SaveState>({ status: 'idle' })

  async function handleSave() {
    const vlanId = vlan === '' ? null : Number(vlan)
    if (vlanId !== null && !Number.isInteger(vlanId)) {
      setSave({ status: 'error', message: 'VLAN must be a whole number.' })
      return
    }

    const changes: UpdateSubnetInput = {}
    if (name !== subnet.name) {
      changes.name = name
    }
    if (description !== subnet.description) {
      changes.description = description
    }
    if (vlanId !== subnet.vlan_id) {
      changes.vlan_id = vlanId
    }

    if (Object.keys(changes).length === 0) {
      onCancel()
      return
    }

    setSave({ status: 'saving' })

    try {
      await updateSubnet(subnet.id, changes)
    } catch (error: unknown) {
      const message =
        error instanceof ApiError ? error.message : 'Could not reach the NetLedger server.'
      setSave({ status: 'error', message })
      return
    }

    onSaved()
  }

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === 'Enter') {
      event.preventDefault()
      void handleSave()
    } else if (event.key === 'Escape') {
      onCancel()
    }
  }

  const saving = save.status === 'saving'

  return (
    <>
      <tr className="editing">
        <td className="mono cidr-cell" style={{ paddingLeft: `${0.75 + depth * 1.5}rem` }}>
          {depth > 0 && (
            <span className="tree-marker" aria-hidden="true">
              └{' '}
            </span>
          )}
          {subnet.cidr}
        </td>
        <td>
          <input
            aria-label={`Name for ${subnet.cidr}`}
            value={name}
            onChange={(event) => setName(event.target.value)}
            onKeyDown={handleKeyDown}
            maxLength={100}
            required
          />
        </td>
        <td>
          <input
            aria-label={`VLAN for ${subnet.cidr}`}
            type="number"
            value={vlan}
            onChange={(event) => setVlan(event.target.value)}
            onKeyDown={handleKeyDown}
            min={1}
            max={4094}
            step={1}
          />
        </td>
        <td>
          <input
            aria-label={`Description for ${subnet.cidr}`}
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            onKeyDown={handleKeyDown}
            maxLength={1000}
          />
        </td>
        <td className="actions">
          <button type="button" onClick={() => void handleSave()} disabled={saving}>
            {saving ? 'Saving…' : 'Save'}
          </button>
          <button type="button" className="secondary" onClick={onCancel} disabled={saving}>
            Cancel
          </button>
        </td>
      </tr>
      {save.status === 'error' && (
        <tr>
          <td colSpan={5}>
            <p role="alert" className="error">
              {save.message}
            </p>
          </td>
        </tr>
      )}
    </>
  )
}
