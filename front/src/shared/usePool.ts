// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
// [FRONT — Hugo] Lecture des chiffres du pool, rafraîchis toutes les 15 s.
// Si le RPC ne répond pas, on affiche des chiffres de démo (signalés par `demo: true`).
import { useCallback, useEffect, useState } from 'react'
import { readActivity, type Activity } from './activity'
import { readStats, toUnits, type PoolStats } from './contract'

const DEMO: PoolStats = {
  totalAssets: toUnits(1_000_000),
  capacity: toUnits(944_987),
  sharePrice: toUnits(1.0042),
  exitPrice: toUnits(0.9493),
  exposures: [
    { currency: 'MXN', exp: { buy_notional: toUnits(550_128), sell_notional: toUnits(0), net_notional: toUnits(550_128), buy_margin: toUnits(55_013), sell_margin: 0n } },
  ],
}

export function usePoolStats() {
  const [stats, setStats] = useState<PoolStats | null>(null)
  const [demo, setDemo] = useState(false)

  const refresh = useCallback(async () => {
    try {
      setStats(await readStats())
      setDemo(false)
    } catch (e) {
      console.warn('[pool] lecture impossible, données de démo', e)
      setStats((s) => s ?? DEMO)
      setDemo(true)
    }
  }, [])

  useEffect(() => {
    refresh()
    const id = setInterval(refresh, 15_000)
    return () => clearInterval(id)
  }, [refresh])

  return { stats, demo, refresh }
}

/** Statistiques d'usage (fournisseurs, PME, volume…), plus lentes à lire : rafraîchies toutes les 30 s. */
export function useActivity() {
  const [activity, setActivity] = useState<Activity | null>(null)
  const [failed, setFailed] = useState(false)

  const refresh = useCallback(async () => {
    try {
      setActivity(await readActivity())
      setFailed(false)
    } catch (e) {
      console.warn('[activity] lecture impossible', e)
      setFailed(true)
    }
  }, [])

  useEffect(() => {
    refresh()
    const id = setInterval(refresh, 30_000)
    return () => clearInterval(id)
  }, [refresh])

  return { activity, failed, refresh }
}
