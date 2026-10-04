import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { TokensPage } from './TokensPage.tsx'

function jsonResponse(status: number, body: unknown) {
  return new Response(body === undefined ? null : JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  })
}

const existing = {
  id: 't1',
  name: 'pipeline',
  hint: 'Ab9Q',
  created_at: '2026-10-04T12:00:00Z',
  expires_at: null,
  last_used_at: null,
  revoked_at: null,
}

describe('TokensPage', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('lists tokens by hint and never by value', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(jsonResponse(200, [existing])))
    render(<TokensPage onBack={vi.fn()} />)

    expect(await screen.findByText('nlt_…Ab9Q')).toBeInTheDocument()
    expect(screen.getByText('pipeline')).toBeInTheDocument()
    expect(screen.getByText('Active')).toBeInTheDocument()
  })

  it('shows a new token once and sends what the form collected', async () => {
    const secret = 'nlt_' + 'x'.repeat(39) + 'Zq1W'
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(jsonResponse(200, []))
      .mockResolvedValueOnce(jsonResponse(201, { ...existing, hint: 'Zq1W', secret }))
      .mockResolvedValueOnce(jsonResponse(200, [{ ...existing, hint: 'Zq1W' }]))
    vi.stubGlobal('fetch', fetchMock)
    render(<TokensPage onBack={vi.fn()} />)

    const user = userEvent.setup()
    await user.type(screen.getByLabelText('Name'), 'pipeline')
    await user.type(screen.getByLabelText('Expires in (days)'), '30')
    await user.click(screen.getByRole('button', { name: 'Create token' }))

    expect(await screen.findByRole('status')).toHaveTextContent('Copy this token now')
    expect(screen.getByText(secret)).toBeInTheDocument()
    const [url, init] = fetchMock.mock.calls[1] as [string, RequestInit]
    expect(url).toBe('/api/tokens')
    expect(JSON.parse(init.body as string)).toEqual({ name: 'pipeline', expires_in_days: 30 })

    await user.click(screen.getByRole('button', { name: 'Done' }))
    expect(screen.queryByText(secret)).not.toBeInTheDocument()
  })

  it('revokes a token after confirmation', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(jsonResponse(200, [existing]))
      .mockResolvedValueOnce(jsonResponse(204, undefined))
      .mockResolvedValueOnce(
        jsonResponse(200, [{ ...existing, revoked_at: '2026-10-04T13:00:00Z' }]),
      )
    vi.stubGlobal('fetch', fetchMock)
    vi.stubGlobal('confirm', vi.fn().mockReturnValue(true))
    render(<TokensPage onBack={vi.fn()} />)

    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: 'Revoke pipeline' }))

    await waitFor(() => expect(screen.getByText('Revoked')).toBeInTheDocument())
    const [url, init] = fetchMock.mock.calls[1] as [string, RequestInit]
    expect(url).toBe('/api/tokens/t1')
    expect(init.method).toBe('DELETE')
    expect(screen.queryByRole('button', { name: 'Revoke pipeline' })).not.toBeInTheDocument()
  })

  it('goes back to the subnets', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(jsonResponse(200, [])))
    const onBack = vi.fn()
    render(<TokensPage onBack={onBack} />)

    await userEvent.setup().click(screen.getByRole('button', { name: '← Back to subnets' }))

    expect(onBack).toHaveBeenCalled()
  })
})
