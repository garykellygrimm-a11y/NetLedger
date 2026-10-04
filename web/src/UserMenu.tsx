import { useEffect, useId, useRef, useState } from 'react'
import type { CurrentUser } from './types.ts'

type Props = {
  user: CurrentUser
  onOpenTokens: () => void
  onSignOut: () => void
}

export function UserMenu({ user, onOpenTokens, onSignOut }: Props) {
  const [open, setOpen] = useState(false)
  const menuId = useId()
  const root = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) {
      return
    }
    function handlePointer(event: MouseEvent) {
      if (root.current && !root.current.contains(event.target as Node)) {
        setOpen(false)
      }
    }
    function handleKey(event: KeyboardEvent) {
      if (event.key === 'Escape') {
        setOpen(false)
      }
    }
    document.addEventListener('mousedown', handlePointer)
    document.addEventListener('keydown', handleKey)
    return () => {
      document.removeEventListener('mousedown', handlePointer)
      document.removeEventListener('keydown', handleKey)
    }
  }, [open])

  function choose(action: () => void) {
    setOpen(false)
    action()
  }

  return (
    <div className="user-menu" ref={root}>
      <button
        type="button"
        className="secondary"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={menuId}
        onClick={() => setOpen((shown) => !shown)}
      >
        {user.username} <span className="role">({user.role})</span>
      </button>
      {open && (
        <ul id={menuId} role="menu" className="user-menu-list">
          <li role="none">
            <button type="button" role="menuitem" onClick={() => choose(onOpenTokens)}>
              API tokens
            </button>
          </li>
          <li role="none">
            <button type="button" role="menuitem" onClick={() => choose(onSignOut)}>
              Sign out
            </button>
          </li>
        </ul>
      )}
    </div>
  )
}
