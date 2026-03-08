import type cytoscape from "cytoscape";

/** Configuration for edge particle animation. */
interface AnimationConfig {
  /** Callback returning whether animation is enabled. */
  isEnabled: () => boolean;
  /** Edge kinds to animate. */
  animatedKinds: Set<string>;
}

/** Particle state for a single edge. */
interface Particle {
  offset: number;
  speed: number;
}

/** Speed multipliers per edge kind. */
const SPEED: Record<string, number> = {
  calls: 0.004,
  data_flow: 0.002,
};

/** Dot radius per edge kind. */
const DOT_RADIUS: Record<string, number> = {
  calls: 2,
  data_flow: 2.5,
};

/** Dot color per edge kind (fallback). */
const DOT_COLOR: Record<string, string> = {
  calls: "rgba(76, 175, 80, 0.9)",
  data_flow: "rgba(76, 175, 80, 0.85)",
};

/**
 * Start particle animation on a canvas overlay.
 * Returns a cleanup function that stops the animation and removes the canvas.
 */
export function startAnimation(
  cy: cytoscape.Core,
  container: HTMLElement,
  config: AnimationConfig,
): () => void {
  const canvas = document.createElement("canvas");
  canvas.style.position = "absolute";
  canvas.style.top = "0";
  canvas.style.left = "0";
  canvas.style.width = "100%";
  canvas.style.height = "100%";
  canvas.style.pointerEvents = "none";
  canvas.style.zIndex = "10";
  container.appendChild(canvas);

  const ctx = canvas.getContext("2d");
  if (!ctx) {
    canvas.remove();
    return () => {};
  }

  // Track particles per edge
  const particles = new Map<string, Particle[]>();
  let animFrame = 0;
  let running = true;

  function resizeCanvas() {
    const rect = container.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);
  }

  resizeCanvas();

  const resizeObserver = new ResizeObserver(resizeCanvas);
  resizeObserver.observe(container);

  function getOrCreateParticles(edgeId: string, speed: number): Particle[] {
    let p = particles.get(edgeId);
    if (!p) {
      // 2-3 particles per edge, staggered
      const count = 2 + Math.floor(Math.random() * 2);
      p = [];
      for (let i = 0; i < count; i++) {
        p.push({ offset: i / count, speed });
      }
      particles.set(edgeId, p);
    }
    return p;
  }

  function animate() {
    if (!running) return;

    if (!config.isEnabled()) {
      animFrame = requestAnimationFrame(animate);
      return;
    }

    const rect = container.getBoundingClientRect();
    ctx!.clearRect(0, 0, rect.width, rect.height);

    const pan = cy.pan();
    const zoom = cy.zoom();

    const edges = cy.edges(":visible");

    edges.forEach((edge) => {
      const kind = edge.data("kind") as string;
      if (!config.animatedKinds.has(kind)) return;

      const speed = SPEED[kind] ?? 0.003;
      const radius = DOT_RADIUS[kind] ?? 2;
      const color = DOT_COLOR[kind] ?? "rgba(150,150,150,0.6)";

      const sourcePos = edge.source().position();
      const targetPos = edge.target().position();

      // Convert model positions to rendered positions
      const sx = sourcePos.x * zoom + pan.x;
      const sy = sourcePos.y * zoom + pan.y;
      const tx = targetPos.x * zoom + pan.x;
      const ty = targetPos.y * zoom + pan.y;

      const edgeParticles = getOrCreateParticles(edge.id(), speed);

      for (const particle of edgeParticles) {
        particle.offset = (particle.offset + particle.speed) % 1;
        const t = particle.offset;
        const x = sx + (tx - sx) * t;
        const y = sy + (ty - sy) * t;

        ctx!.beginPath();
        ctx!.arc(x, y, radius, 0, Math.PI * 2);
        ctx!.fillStyle = color;
        ctx!.fill();
      }
    });

    animFrame = requestAnimationFrame(animate);
  }

  animFrame = requestAnimationFrame(animate);

  return () => {
    running = false;
    cancelAnimationFrame(animFrame);
    resizeObserver.disconnect();
    canvas.remove();
    particles.clear();
  };
}
