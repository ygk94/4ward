// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
// [FRONT — Hugo] Ouvrir la page vault directement sur un rôle (liens de la page About).
import { go } from '../shared/router'

export type Role = 'lp' | 'sme' | null

let pending: Role = null

export function openVault(role: Role) {
  pending = role
  go('/vault')
}

/** Rôle demandé par le dernier lien ; lu une fois au montage de la page vault. */
export function pendingRole(): Role {
  return pending
}

export function clearPendingRole() {
  pending = null
}
