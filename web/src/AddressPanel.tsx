import { useEffect, useState } from 'react'
import { AddressForm } from './AddressForm.tsx'
import { ApiError, deleteAddress, listSubnetAddresses } from './api.ts'
import { usableAddressCount } from './capacity.ts'
import type { Address, Subnet } from './types.ts'

type Props = {
  subnet: Subnet
  hasChildren: boolean
  version: number
  canEdit: boolean
  onClose: () => void
}

type LoadState =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'loaded'; addresses: Address[] }

type ActionState =
  | { status: 'idle' }
  | { status: 'deleting'; id: string }
  | { status: 'error'; message: string }

const SOURCE_LABELS: Record<Address['source'], string> = {
  manual: 'Manual',
  allocated: 'Allocated',
  discovered: 'Discovered',
}

export function AddressPanel({ subnet, hasChildren, version, canEdit, onClose }: Props) {
  const [state, setState] = useState<LoadState>({ status: 'loading' })
  const [action, setAction] = useState<ActionState>({ status: 'idle' })
  const [refreshKey, setRefreshKey] = useState(0)

  useEffect(() => {
    const controller = new AbortController()

    listSubnetAddresses(subnet.id, controller.signal)
      .then((addresses) => setState({ status: 'loaded', addresses }))
      .catch((error: unknown) => {
        if (controller.signal.aborted) {
          return
        }
        const message =
          error instanceof ApiError ? error.message : 'Could not reach the NetLedger server.'
        setState({ status: 'error', message })
      })

    return () => controller.abort()
  }, [subnet.id, version, refreshKey])

  function refresh() {
    setRefreshKey((key) => key + 1)
  }

  async function handleDelete(address: Address) {
    if (!window.confirm(`Delete ${address.address}? This cannot be undone.`)) {
      return
    }

    setAction({ status: 'deleting', id: address.id })

    try {
      await deleteAddress(address.id)
    } catch (error: unknown) {
      const message =
        error instanceof ApiError ? error.message : 'Could not reach the NetLedger server.'
      setAction({ status: 'error', message })
      return
    }

    setAction({ status: 'idle' })
    refresh()
  }

  const capacity = usableAddressCount(subnet.cidr)
  const used = state.status === 'loaded' ? state.addresses.length : null

  return (
    <section className="address-panel" aria-labelledby="addresses-heading">
      <div className="panel-header">
        <h2 id="addresses-heading">
          Addresses in <span className="mono">{subnet.cidr}</span> ({subnet.name})
        </h2>
        <button type="button" className="secondary" onClick={onClose}>
          Close
        </button>
      </div>

      {used !== null && (
        <div className="utilization">
          {capacity !== null && !hasChildren ? (
            <>
              <meter min={0} max={capacity} value={used} aria-label="Addresses recorded" />
              <span>
                {used.toLocaleString()} of {capacity.toLocaleString()} usable addresses recorded
              </span>
            </>
          ) : (
            <span>
              {used.toLocaleString()} recorded directly in this subnet
              {hasChildren && '; addresses inside child subnets are listed under each child'}
            </span>
          )}
        </div>
      )}

      {canEdit && (
        <AddressForm
          subnetId={subnet.id}
          onChanged={() => {
            setAction({ status: 'idle' })
            refresh()
          }}
        />
      )}

      {action.status === 'error' && (
        <p role="alert" className="error">
          {action.message}
        </p>
      )}
      {state.status === 'loading' && <p>Loading addresses…</p>}
      {state.status === 'error' && (
        <p role="alert" className="error">
          {state.message}
        </p>
      )}
      {state.status === 'loaded' && state.addresses.length === 0 && (
        <p className="empty">No addresses recorded in this subnet yet.</p>
      )}
      {state.status === 'loaded' && state.addresses.length > 0 && (
        <table>
          <thead>
            <tr>
              <th scope="col">Address</th>
              <th scope="col">Hostname</th>
              <th scope="col">Description</th>
              <th scope="col">Source</th>
              {canEdit && (
                <th scope="col">
                  <span className="visually-hidden">Actions</span>
                </th>
              )}
            </tr>
          </thead>
          <tbody>
            {state.addresses.map((address) => (
              <tr key={address.id}>
                <td className="mono">{address.address}</td>
                <td>{address.hostname || '—'}</td>
                <td>{address.description}</td>
                <td className="source">{SOURCE_LABELS[address.source]}</td>
                {canEdit && (
                  <td className="actions">
                    <button
                      type="button"
                      className="danger"
                      onClick={() => void handleDelete(address)}
                      disabled={action.status === 'deleting'}
                      aria-label={`Delete ${address.address}`}
                    >
                      {action.status === 'deleting' && action.id === address.id
                        ? 'Deleting…'
                        : 'Delete'}
                    </button>
                  </td>
                )}
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  )
}
