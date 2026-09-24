import { useState } from 'react'
import type { SubmitEvent } from 'react'
import { ApiError, createSubnet } from './api.ts'
import type { Subnet } from './types.ts'

type Props = {
  subnets: Subnet[]
  onCreated: () => void
}

type SubmitState =
  | { status: 'idle' }
  | { status: 'submitting' }
  | { status: 'error'; message: string }

export function SubnetForm({ subnets, onCreated }: Props) {
  const [cidr, setCidr] = useState('')
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [vlan, setVlan] = useState('')
  const [parentId, setParentId] = useState('')
  const [submit, setSubmit] = useState<SubmitState>({ status: 'idle' })

  async function handleSubmit(event: SubmitEvent<HTMLFormElement>) {
    event.preventDefault()
    setSubmit({ status: 'submitting' })

    try {
      await createSubnet({
        cidr: cidr.trim(),
        name,
        description,
        vlan_id: vlan === '' ? null : Number(vlan),
        parent_id: parentId === '' ? null : parentId,
      })
    } catch (error: unknown) {
      const message =
        error instanceof ApiError ? error.message : 'Could not reach the NetLedger server.'
      setSubmit({ status: 'error', message })
      return
    }

    setCidr('')
    setName('')
    setDescription('')
    setVlan('')
    setParentId('')
    setSubmit({ status: 'idle' })
    onCreated()
  }

  const submitting = submit.status === 'submitting'

  return (
    <form className="subnet-form" onSubmit={handleSubmit}>
      <h2>Add a subnet</h2>

      <div className="field">
        <label htmlFor="subnet-cidr">Network (CIDR)</label>
        <input
          id="subnet-cidr"
          className="mono"
          value={cidr}
          onChange={(event) => setCidr(event.target.value)}
          placeholder="10.0.0.0/24"
          required
          autoComplete="off"
          spellCheck={false}
        />
      </div>

      <div className="field">
        <label htmlFor="subnet-name">Name</label>
        <input
          id="subnet-name"
          value={name}
          onChange={(event) => setName(event.target.value)}
          required
          maxLength={100}
        />
      </div>

      <div className="field">
        <label htmlFor="subnet-vlan">VLAN (optional)</label>
        <input
          id="subnet-vlan"
          type="number"
          value={vlan}
          onChange={(event) => setVlan(event.target.value)}
          min={1}
          max={4094}
          step={1}
        />
      </div>

      <div className="field">
        <label htmlFor="subnet-parent">Parent (optional)</label>
        <select
          id="subnet-parent"
          value={parentId}
          onChange={(event) => setParentId(event.target.value)}
        >
          <option value="">None</option>
          {subnets.map((subnet) => (
            <option key={subnet.id} value={subnet.id}>
              {subnet.cidr} ({subnet.name})
            </option>
          ))}
        </select>
      </div>

      <div className="field field-wide">
        <label htmlFor="subnet-description">Description (optional)</label>
        <input
          id="subnet-description"
          value={description}
          onChange={(event) => setDescription(event.target.value)}
          maxLength={1000}
        />
      </div>

      {submit.status === 'error' && (
        <p role="alert" className="error field-wide">
          {submit.message}
        </p>
      )}

      <div className="field-wide">
        <button type="submit" disabled={submitting}>
          {submitting ? 'Adding…' : 'Add subnet'}
        </button>
      </div>
    </form>
  )
}
