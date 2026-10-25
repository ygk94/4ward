// [FRONT — Hugo] Connexion wallet via Stellar Wallets Kit (Freighter, xBull, Albedo, Lobstr, Hana…).
import { StellarWalletsKit } from '@creit.tech/stellar-wallets-kit/sdk'
import { defaultModules } from '@creit.tech/stellar-wallets-kit/modules/utils'
import { Networks, type SwkAppTheme } from '@creit.tech/stellar-wallets-kit/types'
import { useSyncExternalStore } from 'react'
import { config } from './config'

const theme: SwkAppTheme = {
  background: '#fbf8f1',
  'background-secondary': '#f1ebdd',
  'foreground-strong': '#23252e',
  foreground: '#3a3d48',
  'foreground-secondary': '#7a7f8e',
  primary: '#2f6bff',
  'primary-foreground': '#ffffff',
  transparent: 'rgba(0,0,0,0)',
  lighter: '#f6f1e6',
  light: '#ece5d6',
  'light-gray': '#e2dac8',
  gray: '#7a7f8e',
  danger: '#6a58f0',
  border: 'rgba(35,37,46,0.12)',
  shadow: '0 20px 60px rgba(47,107,255,0.15)',
  'border-radius': '14px',
  'font-family': "'Space Grotesk', system-ui, sans-serif",
}

StellarWalletsKit.init({ modules: defaultModules(), network: Networks.TESTNET, theme })

const STORAGE_KEY = 'fx.wallet.address'
let address: string | undefined = readStored()
const listeners = new Set<() => void>()

function readStored() {
  try {
    return localStorage.getItem(STORAGE_KEY) ?? undefined
  } catch {
    return undefined
  }
}

function setAddress(next: string | undefined) {
  address = next
  try {
    if (next) localStorage.setItem(STORAGE_KEY, next)
    else localStorage.removeItem(STORAGE_KEY)
  } catch {
    // stockage indisponible : la session reste en mémoire
  }
  listeners.forEach((l) => l())
}

/** Ouvre la fenêtre de choix du wallet et retourne l'adresse connectée. */
export async function connect(): Promise<string> {
  const { address } = await StellarWalletsKit.authModal()
  setAddress(address)
  return address
}

export async function disconnect() {
  await StellarWalletsKit.disconnect()
  setAddress(undefined)
}

export function getAddress() {
  return address
}

/** Signature d'une transaction : ouvre le wallet pour validation. */
export async function signTransaction(xdr: string, opts?: { networkPassphrase?: string; address?: string }) {
  return StellarWalletsKit.signTransaction(xdr, {
    networkPassphrase: opts?.networkPassphrase ?? config.networkPassphrase,
    address: opts?.address ?? address,
  })
}

export function useWallet() {
  const current = useSyncExternalStore(
    (l) => {
      listeners.add(l)
      return () => listeners.delete(l)
    },
    () => address,
  )
  return { address: current, connect, disconnect }
}

export function shortAddress(a: string) {
  return `${a.slice(0, 4)}…${a.slice(-4)}`
}
