import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { UserMenu } from './UserMenu.tsx'

const gary = { account_id: 'a1', username: 'gary', role: 'administrator' as const }

describe('UserMenu', () => {
  it('opens on click and runs the chosen action', async () => {
    const onOpenTokens = vi.fn()
    const onSignOut = vi.fn()
    render(<UserMenu user={gary} onOpenTokens={onOpenTokens} onSignOut={onSignOut} />)
    const user = userEvent.setup()

    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /gary/ }))
    expect(screen.getByRole('menu')).toBeInTheDocument()

    await user.click(screen.getByRole('menuitem', { name: 'API tokens' }))
    expect(onOpenTokens).toHaveBeenCalled()
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
    expect(onSignOut).not.toHaveBeenCalled()
  })

  it('closes on Escape', async () => {
    render(<UserMenu user={gary} onOpenTokens={vi.fn()} onSignOut={vi.fn()} />)
    const user = userEvent.setup()

    await user.click(screen.getByRole('button', { name: /gary/ }))
    await user.keyboard('{Escape}')

    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
  })
})
