export function usableAddressCount(cidr: string): number | null {
  const [address, prefixText] = cidr.split('/')
  if (address.includes(':')) {
    return null
  }
  const prefix = Number(prefixText)
  const size = 2 ** (32 - prefix)
  return prefix >= 31 ? size : size - 2
}
