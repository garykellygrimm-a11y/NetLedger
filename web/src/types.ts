export type Subnet = {
    id: string
    cidr: string
    name: string
    description: string
    vlan_id: number | null
    parent_id: string | null
    created_at: string
    updated_at: string
}

export type CreateSubnetInput = {
    cidr: string
    name: string
    description: string
    vlan_id: number | null
}

export type UpdateSubnetInput = {
    name?: string
    description?: string
    vlan_id?: number | null
}

export type AddressSource = 'manual' | 'allocated' | 'discovered'

export type Address = {
    id: string
    address: string
    subnet_id: string
    hostname: string
    description: string
    source: AddressSource
    created_at: string
    updated_at: string
}

export type CreateAddressInput = {
    address: string
    hostname: string
    description: string
}

export type AllocateAddressInput = {
    hostname: string
    description: string
}

export type Role = 'viewer' | 'editor' | 'administrator'

export type CurrentUser = {
    account_id: string
    username: string
    role: Role
}

export type SignInInput = {
    username: string
    password: string
}
