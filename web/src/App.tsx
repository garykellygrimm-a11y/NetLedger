import { useEffect, useState } from 'react'
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

    setAction({ status: 'idle' })
    refresh()
  }

  const subnets = state.status === 'loaded' ? state.subnets : []
  const deletingId = action.status === 'deleting' ? action.id : null

  return (
    <main>
      <header>
        <h1>NetLedger</h1>
        <p className="subtitle">IP address management</p>
      </header>

      <SubnetForm
        subnets={subnets}
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
          <SubnetTable subnets={state.subnets} deletingId={deletingId} onDelete={handleDelete} />
        )}
      </section>
    </main>
  )
}

export default App
