// Pan, pinch and tap for the level chart.
//
// Runs in the page for the same reason gesture.js does: every event and every DOM change
// answering it crosses the WebView's IPC channel, which a dragging finger outruns. The chart is
// transformed here for the length of the gesture, and Rust hears one message when it ends.
//
// Everything about the chart's geometry is read off the SVG's data attributes, which Rust writes.
// Nothing about the window, the padding or the timeline is known here.

// A re-evaluation binds a fresh `dioxus` channel, leaving the previous listeners sending into a
// dead one. Tearing the old install down first keeps exactly one live set.
if (window.__atlasPlotCleanup) window.__atlasPlotCleanup();

const app = await new Promise((resolve) => {
  const find = () => {
    const el = document.querySelector(".app");
    if (el) resolve(el);
    else requestAnimationFrame(find);
  };
  find();
});

/** Travel under which a gesture was a tap on a day rather than a drag across days, in pixels. */
const TAP = 8;
/** Scaling under which a pinch counts as having held still. */
const STILL = 0.01;
/** How hard a wheel has to turn to zoom, per notch. */
const WHEEL = 0.01;
/** Quiet after a wheel before the window it left is committed. */
const IDLE_MS = 140;
/** Backstop for clearing the gesture's transform, if no redraw arrives to clear it. */
const SETTLE_MS = 600;

const number = (el, name) => Number(el.dataset[name]);

/** The chart's geometry, in the viewBox units its contents are drawn in. */
const measure = (el) => {
  const box = el.getBoundingClientRect();
  const view = number(el, "viewWidth");
  const padLeft = number(el, "padLeft");
  return {
    box,
    padLeft,
    plotted: view - padLeft - number(el, "padRight"),
    // viewBox units per CSS pixel, which is how a pointer's travel becomes a distance in here.
    unit: box.width > 0 ? view / box.width : 0,
    from: number(el, "from"),
    to: number(el, "to"),
    // How far a gesture may reach: the timeline, narrowed to what the chart actually drew. The
    // overscan either side of the window is all there is to bring in; past it the chart empties.
    floor: Math.max(number(el, "floor"), number(el, "drawnFrom")),
    ceiling: Math.min(number(el, "ceiling"), number(el, "drawnTo")),
    minSpan: number(el, "minSpan"),
    maxSpan: number(el, "maxSpan"),
  };
};

/** Where a position on the screen sits, in viewBox units. */
const at = (g, clientX) => (clientX - g.box.left) * g.unit;

/** The day drawn at a position, before the gesture moved anything. */
const dayAt = (g, x) => g.from + ((x - g.padLeft) / g.plotted) * (g.to - g.from);

/**
 * Keeps the window's span and slides it to make it fit inside the reach. `chart.rs` fits the same
 * way when the message lands, against the timeline alone; the reach sits inside that, so a window
 * this has passed is one it leaves alone.
 */
const fit = (g, from, to) => {
  const room = Math.max(g.ceiling - g.floor, g.minSpan);
  const span = Math.min(
    Math.max(to - from, Math.min(g.minSpan, room)),
    Math.min(g.maxSpan, room),
  );
  let start = (from + to) / 2 - span / 2;
  if (start + span > g.ceiling) start = g.ceiling - span;
  if (start < g.floor) start = g.floor;
  return [start, start + span];
};

/**
 * The chart under a position, found by where the pointer fell rather than by what it hit. An SVG
 * answers only where it is painted, and which parts those are is not something to depend on.
 */
const under = (x, y) =>
  [...app.querySelectorAll("[data-plot]")].find((el) => {
    const box = el.getBoundingClientRect();
    return x >= box.left && x <= box.right && y >= box.top && y <= box.bottom;
  }) ?? null;

let plot = null;
let geometry = null;
let settle = null;
let watching = null;
let idle = null;
// The gesture so far, as one horizontal transform: a point x is drawn at x * scaled + panned.
let panned = 0;
let scaled = 1;
// What it had already come to when the pointers last changed, so a pinch beginning mid-pan
// carries the pan rather than throwing it away.
let heldPan = 0;
let heldScale = 1;
const pointers = new Map();

// Looked up on every use rather than held: a redraw replaces these nodes, and a reference taken
// when the gesture began would go on being written to after it had left the document.
const face = () => app.querySelector("[data-plot]");
const moving = () => app.querySelector("[data-plot-layer]");
const labels = () => app.querySelectorAll("[data-plot-dates] text");

/** What the ends of the box now stand over. */
const showing = (g) => {
  const edge = (x) => dayAt(g, (x - panned) / scaled);
  return [edge(g.padLeft), edge(g.padLeft + g.plotted)];
};

/**
 * Holds the gesture to a window it may actually have, and rebuilds the transform from it.
 *
 * A finger has further to travel than the chart has to give, so without this the transform
 * follows it past the end of what was drawn and into empty chart.
 */
const constrain = (g) => {
  const [from, to] = fit(g, ...showing(g));
  const span = g.to - g.from;
  const opening = g.padLeft + ((from - g.from) / span) * g.plotted;
  scaled = span / (to - from);
  panned = g.padLeft - opening * scaled;
};

const draw = () => {
  const target = moving();
  if (!target || !geometry) return;
  constrain(geometry);
  target.setAttribute("transform", `translate(${panned} 0) scale(${scaled} 1)`);
  // The date labels go where the same transform would put them, but as a translation each, so
  // that a pinch spreads them apart without stretching the lettering.
  for (const label of labels()) {
    const x = Number(label.getAttribute("x"));
    label.setAttribute("transform", `translate(${x * (scaled - 1) + panned} 0)`);
  }
};

/** Drops a watch a commit left armed, so its redraw cannot clear a gesture that started since. */
const unhold = () => {
  if (watching) {
    watching.disconnect();
    watching = null;
  }
  if (settle) {
    clearTimeout(settle);
    settle = null;
  }
};

const release = () => {
  unhold();
  moving()?.removeAttribute("transform");
  for (const label of labels()) label.removeAttribute("transform");
  panned = heldPan = 0;
  scaled = heldScale = 1;
};

/** Holds the gesture's transform until the redraw carrying the new window arrives. */
const hold = () => {
  const target = face();
  if (!target) return;
  if (watching) watching.disconnect();
  watching = new MutationObserver(release);
  watching.observe(target, { attributes: true, attributeFilter: ["data-from", "data-to"] });
  if (settle) clearTimeout(settle);
  settle = setTimeout(release, SETTLE_MS);
};

const begin = (target) => {
  // A gesture starting before the last one's redraw arrives would otherwise be cleared by the
  // watch that redraw fires.
  unhold();
  plot = target;
  geometry = measure(target);
  panned = heldPan = 0;
  scaled = heldScale = 1;
  // Nothing laid out yet, so a pointer's travel has no distance to become. Leave it to the page
  // rather than take the gesture and hold still.
  if (!moving() || !(geometry.unit > 0) || !(geometry.plotted > 0)) {
    plot = null;
    return false;
  }
  return true;
};

/** Folds what the gesture has done so far into its base, and restarts from where it stands. */
const rebase = () => {
  heldPan = panned;
  heldScale = scaled;
  for (const pointer of pointers.values()) {
    pointer.fromX = pointer.x;
    pointer.fromY = pointer.y;
  }
};

const follow = () => {
  const active = [...pointers.values()];
  if (!geometry || active.length === 0) return;
  const g = geometry;

  let step = 0;
  let stretch = 1;
  if (active.length === 1) {
    step = (active[0].x - active[0].fromX) * g.unit;
  } else {
    const [a, b] = active;
    stretch = Math.max(Math.abs(a.x - b.x), 1) / Math.max(Math.abs(a.fromX - b.fromX), 1);
    step = at(g, (a.x + b.x) / 2) - at(g, (a.fromX + b.fromX) / 2) * stretch;
  }

  scaled = heldScale * stretch;
  panned = heldPan * stretch + step;
  draw();
};

/** Reports the window the gesture left, or the day a tap landed on. */
const commit = (read) => {
  const g = geometry;
  if (!g) return;
  if (Math.abs(panned) < TAP * g.unit && Math.abs(scaled - 1) < STILL) {
    release();
    if (read !== null) dioxus.send({ from: g.from, to: g.to, read });
    return;
  }
  // The window is read back off what the gesture left on screen. A tap never draws, so this is
  // fitted rather than trusted: only a gesture that drew has been through `constrain`.
  const [from, to] = fit(g, ...showing(g));
  hold();
  dioxus.send({ from, to, read: null });
};

const down = (id, point) => {
  if (pointers.size >= 2) return false;
  if (pointers.size === 0) {
    const target = under(point.clientX, point.clientY);
    if (!target || !begin(target)) return false;
  } else if (!plot) {
    return false;
  }
  pointers.set(id, {
    fromX: point.clientX,
    fromY: point.clientY,
    x: point.clientX,
    y: point.clientY,
  });
  rebase();
  return true;
};

const moved = (id, point) => {
  const pointer = pointers.get(id);
  if (!pointer) return;
  pointer.x = point.clientX;
  pointer.y = point.clientY;
  follow();
};

const up = (id) => {
  const pointer = pointers.get(id);
  if (!pointer) return;
  pointers.delete(id);

  // A pinch losing one finger carries on as a pan under the other.
  if (pointers.size > 0) {
    rebase();
    return;
  }

  const still = Math.hypot(pointer.x - pointer.fromX, pointer.y - pointer.fromY) < TAP;
  const read = still && geometry ? dayAt(geometry, at(geometry, pointer.fromX)) : null;
  commit(read);
};

/**
 * A trackpad's pinch arrives as a wheel held with ctrl, and its two-finger swipe as one across.
 */
const wheel = (event) => {
  const zooming = event.ctrlKey || event.metaKey;
  if (!zooming && Math.abs(event.deltaX) <= Math.abs(event.deltaY)) return;

  const target = under(event.clientX, event.clientY);
  if (!target) return;
  if (idle === null && !begin(target)) return;
  if (!geometry) return;
  event.preventDefault();

  if (zooming) {
    const stretch = Math.exp(-event.deltaY * WHEEL);
    const pivot = at(geometry, event.clientX);
    panned = panned * stretch + pivot * (1 - stretch);
    scaled *= stretch;
  } else {
    panned -= event.deltaX * geometry.unit;
  }
  draw();

  clearTimeout(idle);
  idle = setTimeout(() => {
    idle = null;
    commit(null);
  }, IDLE_MS);
};

// Touch and mouse rather than pointer events, because gesture.js listens on the same element
// through these and two layers reading one finger through different APIs would diverge.
//
// A touch fires compatibility mouse events after itself. `touching` keeps those from being read
// as a second gesture.
const MOUSE = "mouse";
let touching = false;

const touchStart = (event) => {
  touching = true;
  for (const touch of event.changedTouches) {
    down(touch.identifier, touch);
  }
};
const touchMove = (event) => {
  for (const touch of event.changedTouches) {
    moved(touch.identifier, touch);
  }
};
const touchEnd = (event) => {
  for (const touch of event.changedTouches) {
    up(touch.identifier);
  }
};
// A cancelled touch is the platform taking the gesture back, not a finger finishing one. Ending
// it through `up` would report whatever the chart had moved to, and a cancelled touch that had
// barely moved would report as a tap and move the reading.
const touchCancel = () => {
  pointers.clear();
  release();
};

const mouseStart = (event) => {
  if (touching || event.button !== 0) return;
  // Dragging across an SVG otherwise starts a native drag or a text selection, either of which
  // swallows every move after the first. Hence the non-passive listener.
  if (down(MOUSE, event)) event.preventDefault();
};
const mouseMove = (event) => moved(MOUSE, event);
const mouseEnd = () => up(MOUSE);

app.addEventListener("touchstart", touchStart, { passive: true });
app.addEventListener("touchmove", touchMove, { passive: true });
app.addEventListener("touchend", touchEnd, { passive: true });
app.addEventListener("touchcancel", touchCancel, { passive: true });
app.addEventListener("mousedown", mouseStart, { passive: false });
app.addEventListener("mousemove", mouseMove, { passive: true });
app.addEventListener("mouseup", mouseEnd, { passive: true });
// Not passive: a wheel held with ctrl zooms the page unless this one is answered.
app.addEventListener("wheel", wheel, { passive: false });

window.__atlasPlotCleanup = () => {
  app.removeEventListener("touchstart", touchStart);
  app.removeEventListener("touchmove", touchMove);
  app.removeEventListener("touchend", touchEnd);
  app.removeEventListener("touchcancel", touchCancel);
  app.removeEventListener("mousedown", mouseStart);
  app.removeEventListener("mousemove", mouseMove);
  app.removeEventListener("mouseup", mouseEnd);
  app.removeEventListener("wheel", wheel);
  if (idle) clearTimeout(idle);
  idle = null;
  touching = false;
  release();
  pointers.clear();
  plot = null;
  geometry = null;
  window.__atlasPlotCleanup = null;
};
