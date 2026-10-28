// Copyright (c) 2026 Hugo Thomas, Clément Darmon, Lancelot Watelle. All rights reserved. See LICENSE.
// [FRONT — Hugo] Monte-Carlo hors du fil principal : les curseurs restent fluides pendant les 40 000 trajectoires.
import { summarize, type Params } from './montecarlo'

self.onmessage = (e: MessageEvent<{ id: number; params: Params }>) => {
  self.postMessage({ id: e.data.id, summary: summarize(e.data.params) })
}
