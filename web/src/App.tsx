import { useEffect, useState } from 'react'
import { AddressPanel } from './AddressPanel.tsx'
import { ApiError, deleteSubnet, listSubnets } from './api.ts'
import { SubnetForm } from './SubnetForm.tsx'
import { SubnetTable } from './SubnetTable.tsx'
import type { Subnet } from './types.ts'

type LoadState =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'loaded'; subnets: Subnet[] }

type ActionState =
  | { status: 'idle' }
  | { status: 'deleting'; id: string }
  | { status: 'error'; message: string }

function App() {
  const [state, setState] = useState<LoadState>({ status: 'loading' })
  const [action, setAction] = useState<ActionState>({ status: 'idle' })
  const [refreshKey, setRefreshKey] = useState(0)
  const [selectedId, setSelectedId] = useState<string | null>(null)

  useEffect(() => {
    const controller = new AbortController()

    listSubnets(controller.signal)
      .then((subnets) => setState({ status: 'loaded', subnets }))
      .catch((error: unknown) => {
        if (controller.signal.aborted) {
          return
        }
        const message =
          error instanceof ApiError ? error.message : 'Could not reach the NetLedger server.'
        setState({ status: 'error', message })
      })

    return () => controller.abort()
  }, [refreshKey])

  function refresh() {
    setRefreshKey((key) => key + 1)
  }

  async function handleDelete(subnet: Subnet) {
    const confirmed = window.confirm(
      `Delete ${subnet.cidr} (${subnet.name})? This cannot be undone.`,
    )
    if (!confirmed) {
      return
    }

    setAction({ status: 'deleting', id: subnet.id })

    try {
      await deleteSubnet(subnet.id)
    } catch (error: unknown) {
      const message =
        error instanceof ApiError ? error.message : 'Could not reach the NetLedger server.'
      setAction({ status: 'error', message })
      return
    }

    if (subnet.id === selectedId) {
      setSelectedId(null)
    }
    setAction({ status: 'idle' })
    refresh()
  }

  const subnets = state.status === 'loaded' ? state.subnets : []
  const selected = subnets.find((subnet) => subnet.id === selectedId) ?? null
  const deletingId = action.status === 'deleting' ? action.id : null

  return (
    <main>
      <header>
        <h1>NetLedger</h1>
        <p className="subtitle">IP address management</p>
      </header>

      <SubnetForm
        onCreated={() => {
          setAction({ status: 'idle' })
          refresh()
        }}
      />

      <section aria-labelledby="subnets-heading">
        <h2 id="subnets-heading">Subnets</h2>
        {action.status === 'error' && (
          <p role="alert" className="error">
            {action.message}
          </p>
        )}
        {state.status === 'loading' && <p>Loading subnets…</p>}
        {state.status === 'error' && (
          <p role="alert" className="error">
            {state.message}
          </p>
        )}
        {state.status === 'loaded' && (
          <SubnetTable
            subnets={state.subnets}
            deletingId={deletingId}
            selectedId={selectedId}
            onSelect={(subnet) => setSelectedId(subnet.id)}
            onDelete={handleDelete}
            onUpdated={() => {
              setAction({ status: 'idle' })
              refresh()
            }}
          />
        )}
      </section>

      {selected && (
        <AddressPanel
          key={selected.id}
          subnet={selected}
          hasChildren={subnets.some((subnet) => subnet.parent_id === selected.id)}
          version={refreshKey}
          onClose={() => setSelectedId(null)}
        />
      )}
    </main>
  )
}

export default App
