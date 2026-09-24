import type { CreateSubnetInput, Subnet } from './types.ts'

const UNAVAILABLE_STATUSES = new Set([502, 503, 504])

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
    if (UNAVAILABLE_STATUSES.has(response.status)) {
      throw new ApiError(
        response.status,
        'The NetLedger server is not responding. Try again in a moment.',
      )
    }

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

export function createSubnet(input: CreateSubnetInput): Promise<Subnet> {
  return request<Subnet>('/subnets', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(input),
  })
}
