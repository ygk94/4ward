// [FRONT — Hugo] Le vault en polyèdre filaire tracé à l'encre : plusieurs passes de traits qui tremblent,
// avec une légère traînée (le canvas est atténué à chaque image au lieu d'être vidé).
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

function drawStrokes(strokes: Stroke[], verts: THREE.Vector3[], edges: [number, number][], t: number, amp: number) {
  strokes.forEach((s, k) => {
    const attr = s.line.geometry.getAttribute('instanceStart') as THREE.InterleavedBufferAttribute
    const arr = attr.data.array as Float32Array
    const ampK = k === 0 ? amp * 0.3 : amp
    edges.forEach(([i, j], e) => {
      a.copy(verts[i])
      b.copy(verts[j])
      d.subVectors(b, a)
      a.addScaledVector(d, -s.overshoot[e * 2])
      b.addScaledVector(d, s.overshoot[e * 2 + 1])
      const o = e * 6
      for (let c = 0; c < 6; c++) {
        const base = c < 3 ? a.getComponent(c) : b.getComponent(c - 3)
        arr[o + c] = base + Math.sin(t * (0.6 + (c % 3) * 0.17) + s.jitter[o + c]) * ampK
      }
    })
    attr.data.needsUpdate = true
  })
}

export function InkVault({ className }: { className?: string }) {
  const ref = useRef<HTMLDivElement>(null)

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
      camera.updateProjectionMatrix()
      renderer.setSize(width, height)
      renderer.clear()
    }
    const ro = new ResizeObserver(resize)
    ro.observe(host)
    resize()

    const pointer = { x: 0, y: 0 }
    const onMove = (e: PointerEvent) => {
      pointer.x = e.clientX / window.innerWidth - 0.5
      pointer.y = e.clientY / window.innerHeight - 0.5
    }
    window.addEventListener('pointermove', onMove)

    const outerLocal = outer.verts.map((v) => v.clone())
    const innerLocal = inner.verts.map((v) => v.clone())
    const outerPhase = outer.verts.map(() => Math.random() * Math.PI * 2)
    const innerPhase = inner.verts.map(() => Math.random() * Math.PI * 2)
    const start = performance.now()
    let raf = 0

    const frame = () => {
      const t = ((performance.now() - start) / 1000) * (reduced ? 0.3 : 1)

      // Respiration des formes : chaque sommet s'écarte un peu du centre, à son rythme.
      outer.verts.forEach((v, i) => outerLocal[i].copy(v).multiplyScalar(1 + 0.06 * Math.sin(t * 0.55 + outerPhase[i])))
      inner.verts.forEach((v, i) => innerLocal[i].copy(v).multiplyScalar(1 + 0.12 * Math.sin(t * 0.8 + innerPhase[i])))
      outerGroup.rotation.set(0.35 + Math.sin(t * 0.21) * 0.25 + pointer.y * 0.4, t * 0.18 + pointer.x * 0.6, Math.sin(t * 0.13) * 0.15)
      innerGroup.rotation.set(-t * 0.27, -t * 0.22, t * 0.1)

      drawStrokes(outerStrokes, outerLocal, outer.edges, t, 0.03)
      drawStrokes(innerStrokes, innerLocal, inner.edges, t, 0.025)

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
      renderer.dispose()
      renderer.domElement.remove()
    }
  }, [])

  return <div ref={ref} className={`ink-vault ${className ?? ''}`} aria-hidden />
}
