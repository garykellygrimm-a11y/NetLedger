import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { SignInForm } from './SignInForm.tsx'

function respond(status: number, body: unknown) {
  return vi.fn().mockResolvedValue(
    new Response(JSON.stringify(body), {
      status,
      headers: { 'Content-Type': 'application/json' },
    }),
  )
}

describe('SignInForm', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('reports the signed-in user on success', async () => {
    const fetchMock = respond(201, { account_id: 'a1', username: 'gary', role: 'editor' })
    vi.stubGlobal('fetch', fetchMock)
    const onSignedIn = vi.fn()
    render(<SignInForm onSignedIn={onSignedIn} />)

    const user = userEvent.setup()
    await user.type(screen.getByLabelText('Username'), '  Gary ')
    await user.type(screen.getByLabelText('Password'), 'correct horse battery staple')
    await user.click(screen.getByRole('button', { name: 'Sign in' }))

    expect(onSignedIn).toHaveBeenCalledWith({ account_id: 'a1', username: 'gary', role: 'editor' })
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit]
    expect(url).toBe('/api/session')
    expect(JSON.parse(init.body as string)).toEqual({
      username: 'Gary',
      password: 'correct horse battery staple',
    })
  })

  it('shows one message for a rejected password and clears the field', async () => {
    vi.stubGlobal('fetch', respond(401, { error: 'authentication required' }))
    const onSignedIn = vi.fn()
    render(<SignInForm onSignedIn={onSignedIn} />)

    const user = userEvent.setup()
    await user.type(screen.getByLabelText('Username'), 'gary')
    await user.type(screen.getByLabelText('Password'), 'wrong')
    await user.click(screen.getByRole('button', { name: 'Sign in' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('Wrong username or password.')
    expect(screen.getByLabelText('Password')).toHaveValue('')
    expect(screen.getByLabelText('Username')).toHaveValue('gary')
    expect(onSignedIn).not.toHaveBeenCalled()
  })

  it('passes a lockout message through from the server', async () => {
    vi.stubGlobal('fetch', respond(429, { error: 'too many failed attempts; try again later' }))
    render(<SignInForm onSignedIn={vi.fn()} />)

    const user = userEvent.setup()
    await user.type(screen.getByLabelText('Username'), 'gary')
    await user.type(screen.getByLabelText('Password'), 'anything at all')
    await user.click(screen.getByRole('button', { name: 'Sign in' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('too many failed attempts')
  })
})
