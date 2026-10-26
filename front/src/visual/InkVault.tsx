// [FRONT — Hugo] Le vault en polyèdre filaire tracé à l'encre : plusieurs passes de traits qui tremblent,
// avec une légère traînée (le canvas est atténué à chaque image au lieu d'être vidé).
// Option `burst` : lue à chaque image (0 → 1), fait exploser les arêtes vers l'extérieur en tournoyant.
// Option `rich` (vault de fond) : il s'assemble à l'arrivée, une cage géodésique et de la poussière
// l'entourent, un halo respire au centre, l'explosion lance une onde de choc et accélère la rotation.
// Option `chart` (0 → 1, avec `rich`) : chaque arête vole jusqu'à une colonne d'un graphique en bas de
// l'écran, puis le graphique se dessine en colonnes de points sur une grille de points (façon Morpho).
// Au survol, les colonnes proches de la souris se soulèvent et foncent ; les points tremblent comme l'encre.
import { useEffect, useRef } from 'react'
import * as THREE from 'three'
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js'
import { LineSegments2 } from 'three/examples/jsm/lines/LineSegments2.js'
import { LineSegmentsGeometry } from 'three/examples/jsm/lines/LineSegmentsGeometry.js'

interface Shape {
  verts: THREE.Vector3[]
  edges: [number, number][]
}

/** Sommets uniques et arêtes « vraies » (on ignore les diagonales des faces planes). */
function shapeOf(geo: THREE.BufferGeometry): Shape {
  const pos = geo.getAttribute('position')
  const verts: THREE.Vector3[] = []
  const seen = new Map<string, number>()
  const tris: number[] = []
  for (let i = 0; i < pos.count; i++) {
    const v = new THREE.Vector3().fromBufferAttribute(pos, i)
    const key = v.toArray().map((n) => n.toFixed(3)).join(',')
    let j = seen.get(key)
    if (j === undefined) {
      j = verts.length
      verts.push(v)
      seen.set(key, j)
    }
    tris.push(j)
  }
  const normals = new Map<string, THREE.Vector3[]>()
  for (let i = 0; i < tris.length; i += 3) {
    const [a, b, c] = [verts[tris[i]], verts[tris[i + 1]], verts[tris[i + 2]]]
    const n = new THREE.Vector3().subVectors(b, a).cross(new THREE.Vector3().subVectors(c, a)).normalize()
    for (const [p, q] of [
      [tris[i], tris[i + 1]],
      [tris[i + 1], tris[i + 2]],
      [tris[i + 2], tris[i]],
    ]) {
      const k = p < q ? `${p}-${q}` : `${q}-${p}`
      normals.set(k, [...(normals.get(k) ?? []), n])
    }
  }
  const edges: [number, number][] = []
  normals.forEach((ns, k) => {
    if (ns.length < 2 || ns[0].dot(ns[1]) < 0.999) {
      const [a, b] = k.split('-').map(Number)
      edges.push([a, b])
    }
  })
  return { verts, edges }
}

/** Trajectoire d'éclatement de chaque arête : direction depuis le centre, distance, axe et vitesse de rotation. */
interface Fly {
  dir: THREE.Vector3
  dist: number
  axis: THREE.Vector3
  spin: number
}

function makeFlies(shape: Shape, reach: number): Fly[] {
  return shape.edges.map(([i, j]) => {
    const mid = new THREE.Vector3().addVectors(shape.verts[i], shape.verts[j]).multiplyScalar(0.5)
    const rnd = new THREE.Vector3().randomDirection().multiplyScalar(0.6)
    const dir = (mid.lengthSq() < 1e-6 ? rnd : mid.normalize().add(rnd)).normalize()
    return {
      dir,
      dist: reach * (0.7 + Math.random() * 0.8),
      axis: new THREE.Vector3().randomDirection(),
      spin: (Math.random() - 0.5) * 6,
    }
  })
}

interface Stroke {
  line: LineSegments2
  jitter: Float32Array
  overshoot: Float32Array
}

function makeStrokes(segments: number, layers: { color: string; width: number; opacity: number }[], group: THREE.Object3D) {
  return layers.map((l): Stroke => {
    const geo = new LineSegmentsGeometry()
    geo.setPositions(new Float32Array(segments * 6))
    const mat = new LineMaterial({ color: l.color, linewidth: l.width, transparent: true, opacity: l.opacity, depthWrite: false })
    const line = new LineSegments2(geo, mat)
    line.userData.opacity = l.opacity // gardée pour l'atténuer pendant l'éclatement
    line.frustumCulled = false
    group.add(line)
    const jitter = new Float32Array(segments * 6).map(() => Math.random() * Math.PI * 2)
    const overshoot = new Float32Array(segments * 2).map(() => -0.02 + Math.random() * 0.12)
    return { line, jitter, overshoot }
  })
}

const a = new THREE.Vector3()
const b = new THREE.Vector3()
const d = new THREE.Vector3()
const m = new THREE.Vector3()
const q = new THREE.Quaternion()
const v = new THREE.Vector3()

/** Colonnes du graphique : colonne attribuée à chaque arête, abscisses, sommets, bas commun, avancement. */
interface Bars {
  idx: Int32Array
  x: Float32Array
  top: Float32Array
  bottom: number
  c: number
  fade: number
}

function drawStrokes(
  strokes: Stroke[],
  verts: THREE.Vector3[],
  edges: [number, number][],
  t: number,
  amp: number,
  flies?: Fly[],
  burst = 0,
  bars?: Bars,
) {
  strokes.forEach((s, k) => {
    const attr = s.line.geometry.getAttribute('instanceStart') as THREE.InterleavedBufferAttribute
    const arr = attr.data.array as Float32Array
    const ampK = k === 0 ? amp * 0.3 : amp
    ;(s.line.material as LineMaterial).opacity = s.line.userData.opacity * (1 - 0.75 * burst) * (1 - 0.8 * (bars?.fade ?? 0))
    edges.forEach(([i, j], e) => {
      a.copy(verts[i])
      b.copy(verts[j])
      d.subVectors(b, a)
      a.addScaledVector(d, -s.overshoot[e * 2])
      b.addScaledVector(d, s.overshoot[e * 2 + 1])
      if (flies && burst > 0) {
        // l'arête tourne sur son milieu, puis le milieu part vers l'extérieur
        const f = flies[e]
        m.addVectors(a, b).multiplyScalar(0.5)
        q.setFromAxisAngle(f.axis, f.spin * burst)
        a.sub(m).applyQuaternion(q)
        b.sub(m).applyQuaternion(q)
        m.addScaledVector(f.dir, f.dist * burst)
        a.add(m)
        b.add(m)
      }
      if (bars && bars.c > 0) {
        // l'arête rejoint sa colonne : pied en bas, tête au sommet de la colonne
        const n = bars.idx[e]
        a.lerp(v.set(bars.x[n], bars.bottom, 0), bars.c)
        b.lerp(v.set(bars.x[n], bars.top[n], 0), bars.c)
      }
      const o = e * 6
      for (let c = 0; c < 6; c++) {
        const base = c < 3 ? a.getComponent(c) : b.getComponent(c - 3)
        arr[o + c] = base + Math.sin(t * (0.6 + (c % 3) * 0.17) + s.jitter[o + c]) * ampK
      }
    })
    attr.data.needsUpdate = true
  })
}

/** Halo radial (texture dessinée une fois sur un canvas 2D). */
function glowTexture() {
  const c = document.createElement('canvas')
  c.width = c.height = 128
  const g = c.getContext('2d')!
  const grad = g.createRadialGradient(64, 64, 0, 64, 64, 64)
  grad.addColorStop(0, 'rgba(111, 211, 255, 1)')
  grad.addColorStop(0.35, 'rgba(47, 107, 255, 0.45)')
  grad.addColorStop(1, 'rgba(140, 123, 255, 0)')
  g.fillStyle = grad
  g.fillRect(0, 0, 128, 128)
  return new THREE.CanvasTexture(c)
}

interface Props {
  className?: string
  burst?: () => number
  chart?: () => number
  rich?: boolean
}

export function InkVault({ className, burst, chart, rich = false }: Props) {
  const ref = useRef<HTMLDivElement>(null)
  const burstRef = useRef(burst)
  const chartRef = useRef(chart)
  useEffect(() => {
    burstRef.current = burst
    chartRef.current = chart
  }, [burst, chart])

  useEffect(() => {
    const host = ref.current!
    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches

    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, preserveDrawingBuffer: true })
    renderer.autoClear = false
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
    host.appendChild(renderer.domElement)

    const camera = new THREE.PerspectiveCamera(32, 1, 0.1, 100)
    camera.position.set(0, 0, 8)

    // Polyèdre extérieur (le vault) + polyèdre intérieur qui tourne à contre-sens.
    const scene = new THREE.Scene()
    const outerGroup = new THREE.Group()
    const innerGroup = new THREE.Group()
    scene.add(outerGroup, innerGroup)
    const outer = shapeOf(new THREE.IcosahedronGeometry(1.75, 0))
    const inner = shapeOf(new THREE.DodecahedronGeometry(0.95, 0))
    const outerStrokes = makeStrokes(outer.edges.length, [
      { color: '#2f6bff', width: 1.4, opacity: 0.9 },
      { color: '#6fd3ff', width: 0.8, opacity: 0.6 },
      { color: '#6fd3ff', width: 3, opacity: 0.1 },
      { color: '#8c7bff', width: 0.6, opacity: 0.5 },
      { color: '#b9f3ff', width: 0.5, opacity: 0.4 },
      { color: '#8c7bff', width: 0.4, opacity: 0.35 },
    ], outerGroup)
    const innerStrokes = makeStrokes(inner.edges.length, [
      { color: '#8c7bff', width: 0.9, opacity: 0.6 },
      { color: '#8c7bff', width: 0.5, opacity: 0.4 },
    ], innerGroup)
    const outerFlies = makeFlies(outer, 3.2)
    const innerFlies = makeFlies(inner, 2.6)

    // Cage géodésique autour du vault (fond seulement).
    const cageGroup = new THREE.Group()
    const cage = shapeOf(new THREE.IcosahedronGeometry(2.35, 1))
    const cageStrokes = rich
      ? makeStrokes(cage.edges.length, [
          { color: '#6fd3ff', width: 0.7, opacity: 0.28 },
          { color: '#2f6bff', width: 0.4, opacity: 0.16 },
        ], cageGroup)
      : []
    const cageFlies = makeFlies(cage, 4.5)

    // Poussière en orbite : part vers l'extérieur à l'explosion.
    const DUST = rich ? 900 : 0
    const dustBase = new Float32Array(DUST * 3)
    const dustReach = new Float32Array(DUST)
    for (let i = 0; i < DUST; i++) {
      const v = new THREE.Vector3().randomDirection().multiplyScalar(2.1 + Math.random() * 1.6)
      dustBase.set([v.x, v.y, v.z], i * 3)
      dustReach[i] = 1.2 + Math.random() * 2.4
    }
    const dustGeo = new THREE.BufferGeometry()
    dustGeo.setAttribute('position', new THREE.BufferAttribute(new Float32Array(dustBase), 3))
    const dustMat = new THREE.PointsMaterial({ color: '#2f6bff', size: 2, sizeAttenuation: false, transparent: true, opacity: 0.5, depthWrite: false })
    const dust = new THREE.Points(dustGeo, dustMat)
    const dustGroup = new THREE.Group()
    dustGroup.add(dust)

    // Halo au centre et onde de choc (anneau face caméra qui s'agrandit pendant l'explosion).
    const glow = new THREE.Sprite(new THREE.SpriteMaterial({ map: glowTexture(), transparent: true, opacity: 0, depthWrite: false }))
    glow.renderOrder = -1
    const ringPts: number[] = []
    for (let i = 0; i < 128; i++) {
      const [a0, a1] = [(i / 128) * Math.PI * 2, ((i + 1) / 128) * Math.PI * 2]
      ringPts.push(Math.cos(a0), Math.sin(a0), 0, Math.cos(a1), Math.sin(a1), 0)
    }
    const ringGeo = new LineSegmentsGeometry()
    ringGeo.setPositions(ringPts)
    const ringMat = new LineMaterial({ color: '#6fd3ff', linewidth: 2.2, transparent: true, opacity: 0, depthWrite: false })
    const ring = new LineSegments2(ringGeo, ringMat)
    if (rich) scene.add(cageGroup, dustGroup, glow, ring)

    // Graphique : une colonne par arête (ordre mélangé pour que les trajectoires se croisent),
    // hauteurs tirées d'une marche aléatoire qui monte, normalisée entre 0,15 et 1.
    const NB = outer.edges.length + inner.edges.length + cage.edges.length
    const series = new Float32Array(NB)
    let walk = 0
    for (let i = 0; i < NB; i++) {
      walk += (Math.random() - 0.4) * 0.12
      series[i] = walk
    }
    const [lo, hi] = [Math.min(...series), Math.max(...series)]
    series.forEach((h, i) => (series[i] = 0.15 + (0.85 * (h - lo)) / (hi - lo || 1)))
    const perm = Array.from({ length: NB }, (_, i) => i).sort(() => Math.random() - 0.5)
    const colX = new Float32Array(NB)
    const colTop = new Float32Array(NB)
    // les trois formes partagent les mêmes colonnes
    const bars = (from: number, count: number): Bars => ({
      idx: Int32Array.from(perm.slice(from, from + count)),
      x: colX,
      top: colTop,
      bottom: 0,
      c: 0,
      fade: 0,
    })
    const outerBars = bars(0, outer.edges.length)
    const innerBars = bars(outer.edges.length, inner.edges.length)
    const cageBars = bars(outer.edges.length + inner.edges.length, cage.edges.length)

    // Points : grille pâle sur tout l'écran, colonnes du graphique en bleu.
    const ROWS = 70
    const COL_DOTS = Math.ceil(ROWS * 0.62)
    const dotPoints = (count: number, color: string, size: number, perDot = false) => {
      const geo = new THREE.BufferGeometry()
      geo.setAttribute('position', new THREE.BufferAttribute(new Float32Array(count * 3), 3))
      if (perDot) geo.setAttribute('color', new THREE.BufferAttribute(new Float32Array(count * 3), 3))
      const mat = new THREE.PointsMaterial({
        color: perDot ? '#ffffff' : color,
        vertexColors: perDot,
        size,
        sizeAttenuation: false,
        transparent: true,
        opacity: 0,
        depthWrite: false,
      })
      const pts = new THREE.Points(geo, mat)
      pts.frustumCulled = false
      return { geo, mat, pts }
    }
    const pr = Math.min(window.devicePixelRatio, 2)
    const grid = dotPoints(rich ? NB * ROWS : 0, '#23252e', 1.6 * pr)
    const colDots = dotPoints(rich ? NB * COL_DOTS : 0, '#2f6bff', 2.4 * pr, true)
    const DOT_BLUE = new THREE.Color('#2f6bff')
    const DOT_HOT = new THREE.Color('#1b2fa8') // colonnes survolées : bleu plus profond
    const dotColor = new THREE.Color()
    if (rich) scene.add(grid.pts, colDots.pts)

    // Atténuation : multiplie la couleur ET l'alpha du canvas par (1 − opacity) → traînée d'encre.
    const fadeScene = new THREE.Scene()
    const fadeCam = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1)
    fadeScene.add(
      new THREE.Mesh(
        new THREE.PlaneGeometry(2, 2),
        new THREE.MeshBasicMaterial({
          transparent: true,
          opacity: 0.2,
          depthTest: false,
          depthWrite: false,
          blending: THREE.CustomBlending,
          blendEquation: THREE.AddEquation,
          blendSrc: THREE.ZeroFactor,
          blendDst: THREE.OneMinusSrcAlphaFactor,
          blendSrcAlpha: THREE.ZeroFactor,
          blendDstAlpha: THREE.OneMinusSrcAlphaFactor,
        }),
      ),
    )

    const resize = () => {
      const { width, height } = host.getBoundingClientRect()
      camera.aspect = width / height
      camera.position.z = 8 * Math.max(1, 1 / camera.aspect) // en portrait, recule pour garder le vault entier
      camera.updateProjectionMatrix()
      renderer.setSize(width, height)
      renderer.clear()
    }
    const ro = new ResizeObserver(resize)
    ro.observe(host)
    resize()

    const pointer = { x: 0, y: 0, inside: false }
    const onMove = (e: PointerEvent) => {
      pointer.x = e.clientX / window.innerWidth - 0.5
      pointer.y = e.clientY / window.innerHeight - 0.5
      pointer.inside = true
    }
    const onLeave = () => (pointer.inside = false)
    window.addEventListener('pointermove', onMove)
    document.addEventListener('pointerleave', onLeave)

    const outerLocal = outer.verts.map((v) => v.clone())
    const innerLocal = inner.verts.map((v) => v.clone())
    const outerPhase = outer.verts.map(() => Math.random() * Math.PI * 2)
    const innerPhase = inner.verts.map(() => Math.random() * Math.PI * 2)
    const start = performance.now()
    let raf = 0
    // le vault de fond arrive en morceaux et s'assemble
    let blast = rich ? 1 : 0
    let spin = 0
    let last = 0
    let morph = 0
    let hover = 0 // présence de la souris, lissée

    const frame = () => {
      const t = ((performance.now() - start) / 1000) * (reduced ? 0.3 : 1)
      // éclatement lissé : suit la cible sans à-coups, dans les deux sens
      const target = Math.min(1, Math.max(0, burstRef.current?.() ?? 0))
      const dt = Math.min(0.1, t - last)
      last = t
      const opening = target > blast
      blast += (target - blast) * (1 - Math.exp(-dt * 4.5)) // indépendant du nombre d'images par seconde
      const eased = blast * blast * (3 - 2 * blast)
      // la rotation s'emballe quand le vault éclate, beaucoup moins quand il se reforme (retour sur l'accueil)
      spin += dt * (0.18 + eased * (opening ? 2.2 : 0.45))
      // passage au graphique, plus lent que l'explosion
      morph += (Math.min(1, Math.max(0, chartRef.current?.() ?? 0)) - morph) * (1 - Math.exp(-dt * 1.6))
      const ce = morph * morph * (3 - 2 * morph)
      const still = 1 - ce // en graphique, les formes cessent de tourner
      hover += ((pointer.inside ? 1 : 0) - hover) * (1 - Math.exp(-dt * 3))

      // Respiration des formes : chaque sommet s'écarte un peu du centre, à son rythme.
      outer.verts.forEach((v, i) => outerLocal[i].copy(v).multiplyScalar(1 + 0.06 * Math.sin(t * 0.55 + outerPhase[i])))
      inner.verts.forEach((v, i) => innerLocal[i].copy(v).multiplyScalar(1 + 0.12 * Math.sin(t * 0.8 + innerPhase[i])))
      outerGroup.rotation.set(
        (0.35 + Math.sin(t * 0.21) * 0.25 + pointer.y * 0.4) * still,
        (spin + pointer.x * 0.6) * still,
        Math.sin(t * 0.13) * 0.15 * still,
      )
      innerGroup.rotation.set(-t * 0.27 * still, -t * 0.22 * still, t * 0.1 * still)

      let chartBars: [Bars, Bars, Bars] | undefined
      if (rich) {
        // colonnes calées sur ce que voit la caméra dans le plan z = 0
        const h = 2 * camera.position.z * Math.tan(THREE.MathUtils.degToRad(camera.fov / 2))
        const w = h * camera.aspect
        const bottom = -h / 2 + h * 0.04
        // survol : bosse gaussienne autour de l'abscisse de la souris
        const px = pointer.x * w
        const reach = w * 0.07
        const lift = new Float32Array(NB)
        for (let i = 0; i < NB; i++) {
          colX[i] = (-w / 2 + ((i + 0.5) / NB) * w) * 0.96
          lift[i] = hover * Math.exp(-(((colX[i] - px) / reach) ** 2))
          colTop[i] = bottom + h * 0.56 * series[i] * (1 + 0.05 * Math.sin(t * 0.9 + i * 0.35)) + h * 0.14 * lift[i]
        }
        const fade = Math.min(1, Math.max(0, (ce - 0.7) / 0.3))
        for (const bb of [outerBars, innerBars, cageBars]) Object.assign(bb, { bottom, c: ce, fade })
        chartBars = [outerBars, innerBars, cageBars]

        if (ce > 0.01) {
          const dy = h / ROWS
          const g = grid.geo.getAttribute('position') as THREE.BufferAttribute
          const ga = g.array as Float32Array
          const cd = colDots.geo.getAttribute('position') as THREE.BufferAttribute
          const ca = cd.array as Float32Array
          const cc = colDots.geo.getAttribute('color') as THREE.BufferAttribute
          const cca = cc.array as Float32Array
          // tremblement façon encre : chaque point oscille à son rythme
          const amp = h * 0.0012
          for (let i = 0; i < NB; i++) {
            const x = colX[i]
            for (let r = 0; r < ROWS; r++) {
              const k = i * ROWS + r
              ga.set([x + Math.sin(t * 9 + k * 1.7) * amp, -h / 2 + (r + 0.5) * dy + Math.cos(t * 7.3 + k * 2.3) * amp, -0.01], k * 3)
            }
            dotColor.lerpColors(DOT_BLUE, DOT_HOT, lift[i])
            for (let r = 0; r < COL_DOTS; r++) {
              const k = i * COL_DOTS + r
              const y = bottom + r * dy
              // points au-dessus du sommet de la colonne : rangés hors champ
              ca.set([x + Math.sin(t * 11 + k * 1.3) * amp, y <= colTop[i] ? y + Math.cos(t * 8.1 + k * 1.9) * amp : 1e5, 0], k * 3)
              cca.set([dotColor.r, dotColor.g, dotColor.b], k * 3)
            }
          }
          g.needsUpdate = true
          cd.needsUpdate = true
          cc.needsUpdate = true
        }
        const dots = Math.min(1, Math.max(0, (ce - 0.55) / 0.45))
        grid.mat.opacity = 0.14 * dots
        colDots.mat.opacity = 0.75 * dots
      }

      drawStrokes(outerStrokes, outerLocal, outer.edges, t, 0.03, outerFlies, eased, chartBars?.[0])
      drawStrokes(innerStrokes, innerLocal, inner.edges, t, 0.025, innerFlies, eased, chartBars?.[1])
      if (rich) {
        cageGroup.rotation.set((-0.2 + pointer.y * 0.2) * still, (-spin * 0.4 + pointer.x * 0.3) * still, 0.1 * still)
        drawStrokes(cageStrokes, cage.verts, cage.edges, t, 0.02, cageFlies, eased, chartBars?.[2])

        const pos = dustGeo.getAttribute('position') as THREE.BufferAttribute
        const arr = pos.array as Float32Array
        for (let i = 0; i < DUST; i++) {
          const k = 1 + eased * dustReach[i] + 0.04 * Math.sin(t * 0.7 + i)
          arr[i * 3] = dustBase[i * 3] * k
          arr[i * 3 + 1] = dustBase[i * 3 + 1] * k
          arr[i * 3 + 2] = dustBase[i * 3 + 2] * k
        }
        pos.needsUpdate = true
        dustGroup.rotation.set(t * 0.03, spin * 0.25, 0)
        dustMat.opacity = 0.5 * (1 - 0.6 * eased) * still

        const pulse = 1 + 0.08 * Math.sin(t * 1.3)
        glow.scale.setScalar((2.6 + eased * 5) * pulse)
        glow.material.opacity = 0.09 * (1 - 0.5 * eased) * still

        ring.scale.setScalar(0.4 + eased * 5.5)
        ringMat.opacity = Math.sin(Math.PI * eased) * 0.55 * still
      }

      renderer.render(fadeScene, fadeCam)
      renderer.clearDepth()
      renderer.render(scene, camera)
      raf = requestAnimationFrame(frame)
    }
    raf = requestAnimationFrame(frame)

    return () => {
      cancelAnimationFrame(raf)
      ro.disconnect()
      window.removeEventListener('pointermove', onMove)
      document.removeEventListener('pointerleave', onLeave)
      renderer.dispose()
      renderer.domElement.remove()
    }
  }, [rich])

  return <div ref={ref} className={`ink-vault ${className ?? ''}`} aria-hidden />
}
