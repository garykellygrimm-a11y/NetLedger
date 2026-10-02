import { useState } from 'react'
import type { SubmitEvent } from 'react'
import { ApiError, allocateAddress, createAddress } from './api.ts'

type Props = {
  subnetId: string
  onChanged: () => void
}

type SubmitState =
  | { status: 'idle' }
  | { status: 'submitting' }
  | { status: 'done'; message: string }
  | { status: 'error'; message: string }

function messageFor(error: unknown): string {
  return error instanceof ApiError ? error.message : 'Could not reach the NetLedger server.'
}

export function AddressForm({ subnetId, onChanged }: Props) {
  const [address, setAddress] = useState('')
  const [hostname, setHostname] = useState('')
  const [description, setDescription] = useState('')
  const [submit, setSubmit] = useState<SubmitState>({ status: 'idle' })

  function reset(message: string) {
    setAddress('')
    setHostname('')
    setDescription('')
    setSubmit({ status: 'done', message })
    onChanged()
  }

  async function handleRecord(event: SubmitEvent<HTMLFormElement>) {
    event.preventDefault()
    setSubmit({ status: 'submitting' })

    try {
      const recorded = await createAddress({ address: address.trim(), hostname, description })
      reset(`Recorded ${recorded.address}.`)
    } catch (error: unknown) {
      setSubmit({ status: 'error', message: messageFor(error) })
    }
  }

  async function handleAllocate() {
    setSubmit({ status: 'submitting' })

    try {
      const allocated = await allocateAddress(subnetId, { hostname, description })
      reset(`Allocated ${allocated.address}.`)
    } catch (error: unknown) {
      setSubmit({ status: 'error', message: messageFor(error) })
    }
  }

  const submitting = submit.status === 'submitting'

  return (
    <form className="subnet-form" onSubmit={handleRecord}>
      <h2>Add an address</h2>

      <div className="field">
        <label htmlFor="address-value">Address</label>
        <input
          id="address-value"
          className="mono"
          value={address}
          onChange={(event) => setAddress(event.target.value)}
          placeholder="10.0.1.25"
          autoComplete="off"
          spellCheck={false}
        />
      </div>

      <div className="field">
        <label htmlFor="address-hostname">Hostname (optional)</label>
        <input
          id="address-hostname"
          value={hostname}
          onChange={(event) => setHostname(event.target.value)}
          maxLength={253}
          autoComplete="off"
          spellCheck={false}
        />
      </div>

      <div className="field field-wide">
        <label htmlFor="address-description">Description (optional)</label>
        <input
          id="address-description"
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
      {submit.status === 'done' && (
        <p role="status" className="success field-wide">
          {submit.message}
        </p>
      )}

      <div className="form-actions field-wide">
        <button type="submit" disabled={submitting || address.trim() === ''}>
          Record address
        </button>
        <button
          type="button"
          className="secondary"
          onClick={() => void handleAllocate()}
          disabled={submitting}
        >
          Allocate next free address
        </button>
      </div>
    </form>
  )
}
