import type {
  Address,
  AllocateAddressInput,
  CreateAddressInput,
  CreateSubnetInput,
  Subnet,
  UpdateSubnetInput,
} from './types.ts'

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

  if (response.status === 204) {
    return undefined as T
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

export function updateSubnet(id: string, input: UpdateSubnetInput): Promise<Subnet> {
  return request<Subnet>(`/subnets/${encodeURIComponent(id)}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export function deleteSubnet(id: string): Promise<void> {
  return request<void>(`/subnets/${encodeURIComponent(id)}`, { method: 'DELETE' })
}

export function listSubnetAddresses(subnetId: string, signal?: AbortSignal): Promise<Address[]> {
  return request<Address[]>(`/subnets/${encodeURIComponent(subnetId)}/addresses`, { signal })
}

export function createAddress(input: CreateAddressInput): Promise<Address> {
  return request<Address>('/addresses', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export function allocateAddress(subnetId: string, input: AllocateAddressInput): Promise<Address> {
  return request<Address>(`/subnets/${encodeURIComponent(subnetId)}/addresses/allocate`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export function deleteAddress(id: string): Promise<void> {
  return request<void>(`/addresses/${encodeURIComponent(id)}`, { method: 'DELETE' })
}
