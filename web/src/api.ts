import type {
  Address,
  AllocateAddressInput,
  ApiToken,
  CreateAddressInput,
  CreateApiTokenInput,
  CreateSubnetInput,
  CreatedApiToken,
  CurrentUser,
  SignInInput,
  Subnet,
  UpdateSubnetInput,
} from './types.ts'

const UNAVAILABLE_STATUSES = new Set([502, 503, 504])
const SESSION_PATH = '/session'

let onSessionLost: (() => void) | null = null

export function setSessionLostHandler(handler: (() => void) | null) {
  onSessionLost = handler
}

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
    if (response.status === 401 && path !== SESSION_PATH) {
      onSessionLost?.()
    }

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

export function currentSession(signal?: AbortSignal): Promise<CurrentUser> {
  return request<CurrentUser>(SESSION_PATH, { signal })
}

export function signIn(input: SignInInput): Promise<CurrentUser> {
  return request<CurrentUser>(SESSION_PATH, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export function signOut(): Promise<void> {
  return request<void>(SESSION_PATH, { method: 'DELETE' })
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

export function listTokens(signal?: AbortSignal): Promise<ApiToken[]> {
  return request<ApiToken[]>('/tokens', { signal })
}

export function createToken(input: CreateApiTokenInput): Promise<CreatedApiToken> {
  return request<CreatedApiToken>('/tokens', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(input),
  })
}

export function revokeToken(id: string): Promise<void> {
  return request<void>(`/tokens/${id}`, { method: 'DELETE' })
}
