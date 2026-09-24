import type { Subnet } from './types.ts'

export class ApiError extends Error {
  readonly status: number

  constructor(status: number, message: string) {
    super(message)
    this.status = status
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`/api${path}`, init)

  if (!response.ok) {
    const body: unknown = await response.json().catch(() => null)
    const message =
      typeof body === 'object' &&
      body !== null &&
      'error' in body &&
      typeof body.error === 'string'
        ? body.error
        : `request failed with status ${response.status}`
    throw new ApiError(response.status, message)
  }

  return (await response.json()) as T
}

export function listSubnets(signal?: AbortSignal): Promise<Subnet[]> {
  return request<Subnet[]>('/subnets', { signal })
}
