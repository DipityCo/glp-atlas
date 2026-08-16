// Horizontal swipe between the three pages.
//
// Runs in the page rather than in Rust: touch events and the DOM mutations answering them
// each cross the WebView's IPC channel, which a dragging finger outruns. Rust hears one
// message per committed swipe.
//
// Sets `transform` on the plane elements directly. Writing `--pan` on their shared ancestor
// would invalidate style for every star beneath it.

// A re-evaluation binds a fresh `dioxus` channel, leaving the previous listeners sending into
// a dead one. Tearing the old install down first keeps exactly one live set.
if (window.__atlasSwipeCleanup) window.__atlasSwipeCleanup();

const app = await new Promise((resolve) => {
  const find = () => {
    const el = document.querySelector(".app");
    if (el) resolve(el);
    else requestAnimationFrame(find);
  };
  find();
});

/** Fraction of the viewport a swipe must cross to land on the next page. */
const COMMIT = 0.22;
/** Travel before a gesture is committed to an axis, in pixels. */
const SLOP = 12;
/** How much further a swipe must run horizontally than vertically to count as a pan. */
const BIAS = 1.4;
/** Fraction of the finger's travel used past the first and last page. */
const RESISTANCE = 0.3;
/** Fraction of the finger's travel the content column follows. */
const CONTENT_FOLLOW = 0.35;
/** Outlasts the plane transition, after which the stylesheet resolves to the same place. */
const SETTLE_MS = 1300;
/** Controls that own their horizontal drag. A swipe starting inside one never pans the field. */
const OPT_OUT = "[data-swipe='off']";

const number = (el, property) =>
  parseFloat(getComputedStyle(el).getPropertyValue(property)) || 0;

let origin = null;
let planes = [];
let shell = null;
let basePan = 0;
let depth = 0;
let step = 0;
let pages = 3;
let axis = null;
let travel = 0;
let settle = null;

const place = (pan) => {
  for (const { el, parallax } of planes) {
    const x = (1 - pan) * parallax * step;
    const y = depth * parallax * -2.2;
    const scale = 1 + depth * (0.1 + 0.4 * parallax);
    el.style.transform = `translate3d(${x}vw, ${y}vh, 0) scale(${scale})`;
  }
};

const release = () => {
  for (const { el } of planes) el.style.transform = "";
  if (shell) shell.style.transform = "";
};

/**
 * By where the gesture fell rather than by what it hit: an SVG only answers where it is painted,
 * and the chart is mostly not.
 */
const optedOut = (point) =>
  [...app.querySelectorAll(OPT_OUT)].some((el) => {
    const box = el.getBoundingClientRect();
    return (
      point.clientX >= box.left &&
      point.clientX <= box.right &&
      point.clientY >= box.top &&
      point.clientY <= box.bottom
    );
  });

const start = (point) => {
  origin = null;
  if (optedOut(point)) return;
  if (settle) {
    clearTimeout(settle);
    settle = null;
    release();
  }
  origin = { x: point.clientX, y: point.clientY };
  axis = null;
  travel = 0;
  basePan = number(app, "--pan");
  depth = number(app, "--depth");
  step = number(app, "--pan-step");
  pages = Number(app.dataset.pages) || 3;
  shell = document.querySelector(".shell");
  planes = [...app.querySelectorAll(".sky-plane, .sky-haze, .sky-glow")].map((el) => ({
    el,
    parallax: number(el, "--parallax"),
  }));
};

const move = (point) => {
  if (!origin) return;
  const dx = point.clientX - origin.x;
  const dy = point.clientY - origin.y;

  if (axis === null) {
    if (Math.abs(dx) > SLOP && Math.abs(dx) > Math.abs(dy) * BIAS) axis = "x";
    else if (Math.abs(dy) > SLOP) axis = "y";
  }
  if (axis !== "x") return;

  travel = dx;
  // Dragging left walks forward through the field, so pan rises as the finger falls back.
  let offset = -dx / window.innerWidth;
  const landing = basePan + (offset > 0 ? 1 : -1);
  if (landing < 0 || landing > pages - 1) offset *= RESISTANCE;
  offset = Math.max(-1, Math.min(1, offset));

  app.classList.add("dragging");
  place(basePan + offset);
  if (shell) shell.style.transform = `translate3d(${dx * CONTENT_FOLLOW}px, 0, 0)`;
};

// Puts the field back without a page change, for a gesture that stopped being a swipe: the
// platform taking it back, or a second finger arriving. Going through `end` instead would commit
// a navigation from a drag the user never finished, and returning early out of it would leave
// `dragging` on, holding the field transitionless at whatever offset it had reached.
const cancel = () => {
  if (!origin) return;
  origin = null;
  axis = null;
  travel = 0;
  app.classList.remove("dragging");
  release();
};

const end = () => {
  if (!origin) return;
  origin = null;
  app.classList.remove("dragging");
  if (axis !== "x") {
    release();
    return;
  }

  const crossed = -travel / window.innerWidth;
  const target = basePan + (crossed > 0 ? 1 : -1);
  const committed = Math.abs(crossed) > COMMIT && target >= 0 && target <= pages - 1;
  if (shell) shell.style.transform = "";

  if (!committed) {
    release();
    return;
  }

  // Held on an inline transform until Rust has settled `--pan` on the new page.
  place(target);
  settle = setTimeout(() => {
    settle = null;
    release();
  }, SETTLE_MS);
  dioxus.send(crossed > 0 ? 1 : -1);
};

// Touch and mouse rather than pointer events: touch is what carries this on a phone, and the
// mouse half is what lets `make web` drive the same thing in a browser.
//
// A touch fires compatibility mouse events after itself, and unguarded they read as a second
// gesture. They arrive right behind the touch, so the mouse path is ignored for a moment after
// one rather than latched off: a latch would disable the mouse for good on a device that has both.
/** How long after a touch a mouse event is taken to be that touch's echo. */
const TOUCH_TAIL_MS = 500;
let touchedAt = 0;
let mousing = false;

const touchStart = (event) => {
  touchedAt = Date.now();
  if (event.touches.length !== 1) {
    cancel();
    return;
  }
  start(event.touches[0]);
};
const touchMove = (event) => {
  touchedAt = Date.now();
  if (event.touches.length === 1) move(event.touches[0]);
};
const touchEnd = () => {
  touchedAt = Date.now();
  end();
};
const touchCancel = () => {
  touchedAt = Date.now();
  cancel();
};

const mouseStart = (event) => {
  if (Date.now() - touchedAt < TOUCH_TAIL_MS || event.button !== 0) return;
  mousing = true;
  start(event);
};
const mouseMove = (event) => {
  if (mousing) move(event);
};
const mouseEnd = () => {
  if (!mousing) return;
  mousing = false;
  end();
};

app.addEventListener("touchstart", touchStart, { passive: true });
app.addEventListener("touchmove", touchMove, { passive: true });
app.addEventListener("touchend", touchEnd, { passive: true });
app.addEventListener("touchcancel", touchCancel, { passive: true });
app.addEventListener("mousedown", mouseStart, { passive: true });
app.addEventListener("mousemove", mouseMove, { passive: true });
// On the window as well: a button released outside the field would otherwise leave the swipe
// running, and the page would keep panning under a cursor with nothing held.
app.addEventListener("mouseup", mouseEnd, { passive: true });
window.addEventListener("mouseup", mouseEnd, { passive: true });

window.__atlasSwipeCleanup = () => {
  app.removeEventListener("touchstart", touchStart);
  app.removeEventListener("touchmove", touchMove);
  app.removeEventListener("touchend", touchEnd);
  app.removeEventListener("touchcancel", touchCancel);
  app.removeEventListener("mousedown", mouseStart);
  app.removeEventListener("mousemove", mouseMove);
  app.removeEventListener("mouseup", mouseEnd);
  window.removeEventListener("mouseup", mouseEnd);
  mousing = false;
  if (settle) clearTimeout(settle);
  release();
  app.classList.remove("dragging");
  origin = null;
  axis = null;
  travel = 0;
  planes = [];
  shell = null;
  settle = null;
  window.__atlasSwipeCleanup = null;
};
