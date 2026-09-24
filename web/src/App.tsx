import { useEffect, useState } from 'react'
import { ApiError, listSubnets } from './api.ts'
import { SubnetForm } from './SubnetForm.tsx'
import { SubnetTable } from './SubnetTable.tsx'
import type { Subnet } from './types.ts'

type LoadState =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'loaded'; subnets: Subnet[] }

function App() {
  const [state, setState] = useState<LoadState>({ status: 'loading' })
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

  const subnets = state.status === 'loaded' ? state.subnets : []

  return (
    <main>
      <header>
        <h1>NetLedger</h1>
        <p className="subtitle">IP address management</p>
      </header>

      <SubnetForm subnets={subnets} onCreated={() => setRefreshKey((key) => key + 1)} />

      <section aria-labelledby="subnets-heading">
        <h2 id="subnets-heading">Subnets</h2>
        {state.status === 'loading' && <p>Loading subnets…</p>}
        {state.status === 'error' && (
          <p role="alert" className="error">
            {state.message}
          </p>
        )}
        {state.status === 'loaded' && <SubnetTable subnets={state.subnets} />}
      </section>
    </main>
  )
}

export default App
