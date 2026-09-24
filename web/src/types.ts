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
