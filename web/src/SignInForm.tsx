import { useState } from 'react'
import type { SubmitEvent } from 'react'
import { ApiError, signIn } from './api.ts'
import type { CurrentUser } from './types.ts'

type Props = {
  onSignedIn: (user: CurrentUser) => void
}

type SubmitState =
  | { status: 'idle' }
  | { status: 'submitting' }
  | { status: 'error'; message: string }

function describe(error: unknown): string {
  if (!(error instanceof ApiError)) {
    return 'Could not reach the NetLedger server.'
  }
  if (error.status === 401) {
    return 'Wrong username or password.'
  }
  return error.message
}

export function SignInForm({ onSignedIn }: Props) {
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [submit, setSubmit] = useState<SubmitState>({ status: 'idle' })

  async function handleSubmit(event: SubmitEvent<HTMLFormElement>) {
    event.preventDefault()
    setSubmit({ status: 'submitting' })

    let user: CurrentUser
    try {
      user = await signIn({ username: username.trim(), password })
    } catch (error: unknown) {
      setPassword('')
      setSubmit({ status: 'error', message: describe(error) })
      return
    }

    setSubmit({ status: 'idle' })
    onSignedIn(user)
  }

  const submitting = submit.status === 'submitting'

  return (
    <form className="sign-in" onSubmit={handleSubmit} aria-labelledby="sign-in-heading">
      <h2 id="sign-in-heading">Sign in</h2>

      <div className="field">
        <label htmlFor="sign-in-username">Username</label>
        <input
          id="sign-in-username"
          value={username}
          onChange={(event) => setUsername(event.target.value)}
          required
          autoComplete="username"
          autoCapitalize="none"
          spellCheck={false}
        />
      </div>

      <div className="field">
        <label htmlFor="sign-in-password">Password</label>
        <input
          id="sign-in-password"
          type="password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          required
          autoComplete="current-password"
        />
      </div>

      {submit.status === 'error' && (
        <p role="alert" className="error">
          {submit.message}
        </p>
      )}

      <div className="form-actions">
        <button type="submit" disabled={submitting}>
          {submitting ? 'Signing in…' : 'Sign in'}
        </button>
      </div>
    </form>
  )
}
