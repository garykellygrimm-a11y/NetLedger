import { useEffect, useState } from 'react'
import type { SubmitEvent } from 'react'
import { ApiError, createToken, listTokens, revokeToken } from './api.ts'
import type { ApiToken, CreatedApiToken } from './types.ts'

type Props = {
  onBack: () => void
}

type LoadState =
  | { status: 'loading' }
  | { status: 'loaded'; tokens: ApiToken[] }
  | { status: 'error'; message: string }

type ActionState =
  | { status: 'idle' }
  | { status: 'creating' }
  | { status: 'revoking'; id: string }
  | { status: 'error'; message: string }

function describe(error: unknown): string {
  return error instanceof ApiError ? error.message : 'Could not reach the NetLedger server.'
}

function formatDate(value: string | null, fallback: string): string {
  return value ? new Date(value).toLocaleString() : fallback
}

export function TokensPage({ onBack }: Props) {
  const [state, setState] = useState<LoadState>({ status: 'loading' })
  const [action, setAction] = useState<ActionState>({ status: 'idle' })
  const [refreshKey, setRefreshKey] = useState(0)
  const [name, setName] = useState('')
  const [expiresInDays, setExpiresInDays] = useState('')
  const [created, setCreated] = useState<CreatedApiToken | null>(null)
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    const controller = new AbortController()

    listTokens(controller.signal)
      .then((tokens) => setState({ status: 'loaded', tokens }))
      .catch((error: unknown) => {
        if (controller.signal.aborted) {
          return
        }
        setState({ status: 'error', message: describe(error) })
      })

    return () => controller.abort()
  }, [refreshKey])

  function refresh() {
    setRefreshKey((key) => key + 1)
  }

  async function handleCreate(event: SubmitEvent<HTMLFormElement>) {
    event.preventDefault()
    setAction({ status: 'creating' })
    setCreated(null)
    setCopied(false)

    const days = expiresInDays.trim()
    let token: CreatedApiToken
    try {
      token = await createToken({
        name: name.trim(),
        ...(days ? { expires_in_days: Number(days) } : {}),
      })
    } catch (error: unknown) {
      setAction({ status: 'error', message: describe(error) })
      return
    }

    setCreated(token)
    setName('')
    setExpiresInDays('')
    setAction({ status: 'idle' })
    refresh()
  }

  async function handleRevoke(token: ApiToken) {
    const confirmed = window.confirm(
      `Revoke the token "${token.name}" (…${token.hint})? Anything using it will stop working.`,
    )
    if (!confirmed) {
      return
    }

    setAction({ status: 'revoking', id: token.id })
    try {
      await revokeToken(token.id)
    } catch (error: unknown) {
      setAction({ status: 'error', message: describe(error) })
      return
    }
    setAction({ status: 'idle' })
    refresh()
  }

  async function handleCopy() {
    if (!created) {
      return
    }
    try {
      await navigator.clipboard.writeText(created.secret)
      setCopied(true)
    } catch {
      setCopied(false)
    }
  }

  const busy = action.status === 'creating' || action.status === 'revoking'

  return (
    <section aria-labelledby="tokens-heading">
      <nav aria-label="Breadcrumb">
        <button type="button" className="link" onClick={onBack}>
          ← Back to subnets
        </button>
      </nav>
      <h2 id="tokens-heading">API tokens</h2>

      <p className="help">
        Tokens let scripts and pipelines use the API with your role. Send one as{' '}
        <code>Authorization: Bearer nlt_…</code>. A token's value is shown once, when it is
        created; NetLedger stores only a hash of it.
      </p>

      <form onSubmit={handleCreate} aria-label="Create a token" className="stack">
        <div className="field">
          <label htmlFor="token-name">Name</label>
          <input
            id="token-name"
            value={name}
            onChange={(event) => setName(event.target.value)}
            required
            maxLength={100}
            placeholder="Provisioning pipeline"
          />
        </div>
        <div className="field">
          <label htmlFor="token-expires">Expires in (days)</label>
          <input
            id="token-expires"
            type="number"
            min={1}
            max={365}
            value={expiresInDays}
            onChange={(event) => setExpiresInDays(event.target.value)}
            placeholder="Never"
          />
        </div>
        <div className="form-actions">
          <button type="submit" disabled={busy}>
            {action.status === 'creating' ? 'Creating…' : 'Create token'}
          </button>
        </div>
      </form>

      {created && (
        <div role="status" className="token-reveal">
          <p>
            <strong>Copy this token now.</strong> It will not be shown again.
          </p>
          <code className="mono">{created.secret}</code>
          <div className="form-actions">
            <button type="button" className="secondary" onClick={() => void handleCopy()}>
              {copied ? 'Copied' : 'Copy'}
            </button>
            <button type="button" className="secondary" onClick={() => setCreated(null)}>
              Done
            </button>
          </div>
        </div>
      )}

      {action.status === 'error' && (
        <p role="alert" className="error">
          {action.message}
        </p>
      )}
      {state.status === 'loading' && <p>Loading tokens…</p>}
      {state.status === 'error' && (
        <p role="alert" className="error">
          {state.message}
        </p>
      )}
      {state.status === 'loaded' && state.tokens.length === 0 && (
        <p className="empty">You have no API tokens.</p>
      )}
      {state.status === 'loaded' && state.tokens.length > 0 && (
        <table>
          <thead>
            <tr>
              <th scope="col">Name</th>
              <th scope="col">Token</th>
              <th scope="col">Created</th>
              <th scope="col">Expires</th>
              <th scope="col">Last used</th>
              <th scope="col">Status</th>
              <th scope="col">
                <span className="visually-hidden">Actions</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {state.tokens.map((token) => {
              const revoked = token.revoked_at !== null
              const expired =
                !revoked && token.expires_at !== null && new Date(token.expires_at) <= new Date()
              return (
                <tr key={token.id}>
                  <td>{token.name}</td>
                  <td className="mono">nlt_…{token.hint}</td>
                  <td>{formatDate(token.created_at, '—')}</td>
                  <td>{formatDate(token.expires_at, 'Never')}</td>
                  <td>{formatDate(token.last_used_at, 'Never')}</td>
                  <td>{revoked ? 'Revoked' : expired ? 'Expired' : 'Active'}</td>
                  <td className="actions">
                    {!revoked && (
                      <button
                        type="button"
                        className="danger"
                        onClick={() => void handleRevoke(token)}
                        disabled={busy}
                        aria-label={`Revoke ${token.name}`}
                      >
                        {action.status === 'revoking' && action.id === token.id
                          ? 'Revoking…'
                          : 'Revoke'}
                      </button>
                    )}
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      )}
    </section>
  )
}
