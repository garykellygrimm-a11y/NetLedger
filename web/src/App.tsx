import { useEffect, useState } from 'react'
import { AddressPanel } from './AddressPanel.tsx'
import {
  ApiError,
  currentSession,
  deleteSubnet,
  listSubnets,
  setSessionLostHandler,
  signOut,
} from './api.ts'
import { SignInForm } from './SignInForm.tsx'
import { SubnetForm } from './SubnetForm.tsx'
import { SubnetTable } from './SubnetTable.tsx'
import type { CurrentUser, Subnet } from './types.ts'

type SessionState =
  | { status: 'checking' }
  | { status: 'signed-out'; notice: string | null }
  | { status: 'signed-in'; user: CurrentUser }

type LoadState =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'loaded'; subnets: Subnet[] }

type ActionState =
  | { status: 'idle' }
  | { status: 'deleting'; id: string }
  | { status: 'error'; message: string }

function App() {
  const [session, setSession] = useState<SessionState>({ status: 'checking' })

  useEffect(() => {
    setSessionLostHandler(() =>
      setSession({ status: 'signed-out', notice: 'Your session has ended. Sign in again.' }),
    )
    return () => setSessionLostHandler(null)
  }, [])

  useEffect(() => {
    const controller = new AbortController()

    currentSession(controller.signal)
      .then((user) => setSession({ status: 'signed-in', user }))
      .catch((error: unknown) => {
        if (controller.signal.aborted) {
          return
        }
        const notice =
          error instanceof ApiError && error.status === 401
            ? null
            : 'Could not reach the NetLedger server.'
        setSession({ status: 'signed-out', notice })
      })

    return () => controller.abort()
  }, [])

  async function handleSignOut() {
    try {
      await signOut()
    } catch {
      // The cookie is cleared server-side on the next request either way.
    }
    setSession({ status: 'signed-out', notice: null })
  }

  if (session.status === 'checking') {
    return (
      <main>
        <header>
          <h1>NetLedger</h1>
          <p className="subtitle">IP address management</p>
        </header>
        <p>Checking your session…</p>
      </main>
    )
  }

  if (session.status === 'signed-out') {
    return (
      <main>
        <header>
          <h1>NetLedger</h1>
          <p className="subtitle">IP address management</p>
        </header>
        {session.notice && (
          <p role="status" className="error">
            {session.notice}
          </p>
        )}
        <SignInForm onSignedIn={(user) => setSession({ status: 'signed-in', user })} />
      </main>
    )
  }

  return <Workspace user={session.user} onSignOut={handleSignOut} />
}

type WorkspaceProps = {
  user: CurrentUser
  onSignOut: () => void
}

function Workspace({ user, onSignOut }: WorkspaceProps) {
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
  const canEdit = user.role !== 'viewer'

  return (
    <main>
      <header>
        <div>
          <h1>NetLedger</h1>
          <p className="subtitle">IP address management</p>
        </div>
        <div className="session-bar">
          <span>
            Signed in as <strong>{user.username}</strong> ({user.role})
          </span>
          <button type="button" onClick={onSignOut}>
            Sign out
          </button>
        </div>
      </header>

      {canEdit && (
        <SubnetForm
          onCreated={() => {
            setAction({ status: 'idle' })
            refresh()
          }}
        />
      )}

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
            canEdit={canEdit}
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
          canEdit={canEdit}
          onClose={() => setSelectedId(null)}
        />
      )}
    </main>
  )
}

export default App
