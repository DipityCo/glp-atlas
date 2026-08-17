//! The app's stylesheet, injected once at the document root.
//!
//! Two halves: the star field under `.sky`, and the chrome above it. The field's motion is
//! expressed in CSS; Rust supplies only the camera as custom properties.

pub const STYLESHEET: &str = r#"
:root {
  --sky-top: #1e2242;
  --sky-mid: #333865;
  --sky-low: #4c5075;
  --ink: #eef0ff;
  --ink-soft: #b7bcdd;
  --ink-faint: #a8adcf;
  --accent: #9fc0ff;
  --accent-deep: #6f92e0;
  --warm: #f2c48d;
  /* For what deletes something. The lightness of --warm, so it reads as red beside it rather than
     as a brighter alarm, at 4.9:1 over the card. */
  --danger: #f2928c;
  /* One tint per drug. Checked against the card behind them for lightness, chroma, contrast, and
     for telling any two apart under protanopia, deuteranopia and tritanopia. Changing one means
     checking the set again. */
  --tint-semaglutide: #5b8ae8;
  --tint-tirzepatide: #c4832f;
  --tint-retatrutide: #d4569b;
  --line: rgba(196, 204, 255, 0.14);
  --line-strong: rgba(196, 204, 255, 0.26);
  /* Opaque enough to hold --ink-faint above 4.5:1 over the bright end of the sky. */
  --panel: linear-gradient(158deg, rgba(52, 56, 92, 0.88), rgba(28, 31, 55, 0.86));
  --radius: 18px;
  /* Widest the reading column grows to; beyond it the layout centres. */
  --column: 34rem;
  --gutter: 1.25rem;
  /* Lengths that track the display-size setting are in rem; MainActivity ties that to the
     system font scale through WebView's text zoom. */
  --tap: 3rem;
  /* Native controls (date pickers, scrollbars) follow the sky, not the platform. */
  color-scheme: dark;
}

* { box-sizing: border-box; -webkit-tap-highlight-color: transparent; }

html { overscroll-behavior: none; }

body {
  margin: 0;
  background: var(--sky-top);
  color: var(--ink);
  font-family: -apple-system, Roboto, system-ui, sans-serif;
  overscroll-behavior: none;
}

/* The camera at rest: --pan in page units, --depth in sub-page steps, --pan-step the
   distance the front plane covers for one page. A swipe sets the planes' transforms
   directly, since a custom property here inherits into every star below it. */
.app {
  --pan: 0;
  --depth: 0;
  --pan-step: 26vw;
  position: fixed;
  inset: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  /* Vertical scrolling stays with the browser; horizontal is ours. */
  touch-action: pan-y;
}

.sky {
  position: absolute;
  inset: 0;
  overflow: hidden;
  z-index: 0;
  background: linear-gradient(180deg, var(--sky-top) 0%, var(--sky-mid) 46%, var(--sky-low) 100%);
}

/* Each plane carries the camera, on a long eased transition so the field reads as heavier
   than the content in front of it. */
.sky-plane,
.sky-haze,
.sky-glow {
  --parallax: 1;
  position: absolute;
  inset: 0;
  transform:
    translate3d(
      calc((1 - var(--pan)) * var(--parallax) * var(--pan-step)),
      calc(var(--depth) * var(--parallax) * -2.2vh),
      0
    )
    scale(calc(1 + var(--depth) * (0.1 + 0.4 * var(--parallax))));
  opacity: calc(1 - var(--depth) * var(--parallax) * 0.12);
  will-change: transform, opacity;
  transition:
    transform 1150ms cubic-bezier(0.16, 0.84, 0.28, 1),
    opacity 900ms ease;
}

/* Under the finger the camera is positional: a transition would lag the gesture by its own
   duration. It returns on release to carry the field the rest of the way. */
.app.dragging .sky-plane,
.app.dragging .sky-haze,
.app.dragging .sky-glow,
.app.dragging .shell {
  /* Transition only. `animation: none` here would replay the shell's enter keyframes as
     soon as the class came off, since an animation restarts when its name changes from
     `none` back to a name. */
  transition: none;
}

/* Ambient motion, inside a plane so it composes with the camera transform rather than
   interrupting it. */
.drift {
  animation-name: drift;
  animation-timing-function: ease-in-out;
  animation-iteration-count: infinite;
  animation-direction: alternate;
  will-change: transform;
}

/* Sized per plane in sky.rs. */
.sky-field { position: absolute; }

@keyframes drift {
  from { transform: translate3d(calc(var(--dx) * -1), calc(var(--dy) * -1), 0) rotate(calc(var(--rot) * -1)); }
  to   { transform: translate3d(var(--dx), var(--dy), 0) rotate(var(--rot)); }
}

.star {
  position: absolute;
  display: block;
  border-radius: 50%;
  background: var(--tint);
}

.twinkle {
  animation-name: twinkle;
  animation-timing-function: ease-in-out;
  animation-iteration-count: infinite;
  will-change: opacity, transform;
}

@keyframes twinkle {
  0%, 100% { opacity: var(--o-dim); transform: scale(0.86); }
  50%      { opacity: var(--o-bright); transform: scale(1.18); }
}

/* A thin diffraction cross on the brightest few, inheriting the star's animated opacity. */
.flare::before,
.flare::after {
  content: "";
  position: absolute;
  left: 50%;
  top: 50%;
  transform: translate(-50%, -50%);
  opacity: 0.55;
}
.flare::before {
  width: 1000%; height: 1px;
  background: linear-gradient(to right, transparent, var(--tint), transparent);
}
.flare::after {
  width: 1px; height: 1000%;
  background: linear-gradient(to bottom, transparent, var(--tint), transparent);
}

/* A faint galactic band and two soft clouds. Softness comes from wide gradient stops
   rather than blur(), which re-rasterises whenever the plane it sits on moves. */
.haze {
  position: absolute;
  border-radius: 50%;
}
.haze-a {
  left: -12%; top: 4%; width: 78%; height: 42%;
  background: radial-gradient(closest-side circle, rgba(126, 140, 214, 0.22), transparent 78%);
}
.haze-b {
  right: -18%; top: 26%; width: 70%; height: 38%;
  background: radial-gradient(closest-side circle, rgba(178, 146, 196, 0.18), transparent 80%);
}
.milkyway {
  position: absolute;
  left: -30%; top: 6%; width: 160%; height: 54%;
  transform: rotate(-19deg);
  background: radial-gradient(closest-side ellipse, rgba(214, 220, 255, 0.14), transparent 82%);
}

/* A light dome over one point on the horizon rather than an even band, centred below the
   bottom edge so only its upper arc shows. Both blobs fade out well inside their boxes, so
   the plane can move without an edge appearing. */
.glow-field, .haze-field { position: absolute; inset: 0; }
.horizon {
  position: absolute;
  left: -55%; bottom: -44%;
  width: 170%; height: 80%;
  background: radial-gradient(closest-side ellipse, rgba(220, 214, 244, 0.54), transparent 76%);
  pointer-events: none;
}
.afterglow {
  position: absolute;
  left: -11%; bottom: -24%;
  width: 90%; height: 44%;
  background: radial-gradient(closest-side ellipse, rgba(240, 196, 170, 0.34), transparent 74%);
  pointer-events: none;
}
.vignette {
  position: absolute;
  inset: 0;
  background: radial-gradient(132% 94% at 40% 34%, transparent 54%, rgba(13, 14, 30, 0.46) 100%);
  pointer-events: none;
}

.shell {
  position: relative;
  z-index: 1;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  animation-duration: 520ms;
  animation-timing-function: cubic-bezier(0.19, 0.86, 0.3, 1);
  /* `backwards`, not `both`: a forwards fill keeps its final transform applied and would
     outrank the drag transform for the life of the element. */
  animation-fill-mode: backwards;
  /* A swipe sets this transform directly, at a fraction of the finger's travel, landing the
     content near where the enter keyframes start. Releasing clears it. */
  transition: transform 320ms cubic-bezier(0.2, 0.8, 0.3, 1);
  /* Promoted ahead of the enter animation, so its first frame does not also rasterise. */
  will-change: transform, opacity;
}

/* Content moves against the camera: the field slides one way, the page arrives from the
   other. */
.enter-pan-left  { animation-name: enter-pan-left; }
.enter-pan-right { animation-name: enter-pan-right; }
.enter-zoom-in   { animation-name: enter-zoom-in; }
.enter-zoom-out  { animation-name: enter-zoom-out; }

@keyframes enter-pan-left {
  from { opacity: 0; transform: translate3d(38px, 0, 0); }
  to   { opacity: 1; transform: none; }
}
@keyframes enter-pan-right {
  from { opacity: 0; transform: translate3d(-38px, 0, 0); }
  to   { opacity: 1; transform: none; }
}
@keyframes enter-zoom-in {
  from { opacity: 0; transform: scale(0.90); }
  to   { opacity: 1; transform: none; }
}
@keyframes enter-zoom-out {
  from { opacity: 0; transform: scale(1.10); }
  to   { opacity: 1; transform: none; }
}

/* Shared column: padded past display cutouts, capped at --column, centred beyond it. */
.topbar, .column {
  width: 100%;
  max-width: calc(var(--column) + env(safe-area-inset-left, 0px) + env(safe-area-inset-right, 0px));
  margin-inline: auto;
  padding-left: calc(var(--gutter) + env(safe-area-inset-left, 0px));
  padding-right: calc(var(--gutter) + env(safe-area-inset-right, 0px));
}

.topbar {
  padding-top: calc(0.875rem + env(safe-area-inset-top, 0px));
  padding-bottom: 0.75rem;
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
}
.topbar h1 {
  margin: 0;
  font-size: 1.7rem;
  font-weight: 620;
  letter-spacing: -0.02em;
  text-shadow: 0 2px 18px rgba(10, 12, 30, 0.55);
}
.topbar .subtitle {
  margin: 0.1875rem 0 0;
  font-size: 0.8125rem;
  color: var(--ink-soft);
}

.back {
  width: var(--tap);
  height: var(--tap);
  flex: none;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 12px;
  border: 1px solid var(--line);
  background: rgba(255, 255, 255, 0.07);
  color: var(--ink);
  cursor: pointer;
}
.back:active { background: rgba(255, 255, 255, 0.14); }

/* Runs the full width of the field. A scroller clips on both axes (`overflow-x` computes to
   `auto` alongside `overflow-y`) and the cards' shadows reach past any narrower box, so an
   inset edge here would shear them into a rectangle standing over the moving field. The
   reading column is capped on `.column` within. `padding-top` clears the first card's shadow
   above it for the same reason. */
.content {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  -webkit-overflow-scrolling: touch;
  padding-top: 1.5rem;
  padding-bottom: calc(7rem + env(safe-area-inset-bottom, 0px));
}

.column {
  display: flex;
  flex-direction: column;
  gap: 0.875rem;
}

.tabbar {
  position: relative;
  z-index: 2;
  display: flex;
  gap: 0.375rem;
  padding: 0.5rem 0.625rem;
  width: calc(100% - 1.75rem - env(safe-area-inset-left, 0px) - env(safe-area-inset-right, 0px));
  max-width: var(--column);
  margin: 0 auto calc(0.625rem + env(safe-area-inset-bottom, 0px));
  border-radius: 22px;
  border: 1px solid var(--line);
  /* Opaque rather than a backdrop-filter: the sky moves under this bar, and a live blur
     re-snapshots its backdrop every frame. */
  background: linear-gradient(160deg, rgba(62, 66, 108, 0.86), rgba(26, 29, 52, 0.88));
  box-shadow: 0 14px 38px rgba(8, 9, 22, 0.42);
}
.tab {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.25rem;
  padding: 0.625rem 0.25rem;
  border: 0;
  border-radius: 16px;
  background: transparent;
  color: var(--ink-faint);
  font-size: 0.6875rem;
  font-weight: 560;
  letter-spacing: 0.01em;
  cursor: pointer;
  transition: color 260ms ease, background 260ms ease;
}
.tab.active {
  color: var(--ink);
  background: rgba(159, 192, 255, 0.16);
  box-shadow: inset 0 0 0 1px rgba(159, 192, 255, 0.22);
}

.card {
  border-radius: var(--radius);
  border: 1px solid var(--line);
  background: var(--panel);
  box-shadow: 0 16px 40px rgba(8, 9, 22, 0.30);
  padding: 1rem;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}
.card.tight { gap: 0.5rem; }
.card.flush { padding: 0.375rem; gap: 0; }

.card-title {
  margin: 0;
  font-size: 0.75rem;
  font-weight: 620;
  letter-spacing: 0.09em;
  text-transform: uppercase;
  color: var(--ink-faint);
}

.hero {
  flex-direction: row;
  align-items: center;
  gap: 1.125rem;
  border: 1px solid var(--line-strong);
  background:
    radial-gradient(120% 130% at 8% 0%, rgba(159, 192, 255, 0.20), transparent 62%),
    var(--panel);
}
.hero-main { display: flex; flex-direction: column; gap: 0.1875rem; min-width: 0; }
.hero-value {
  font-size: 2.125rem;
  font-weight: 640;
  letter-spacing: -0.03em;
  line-height: 1.05;
  /* Figures update in place; proportional digits jitter. */
  font-variant-numeric: tabular-nums;
}
.hero-unit { font-size: 1rem; font-weight: 520; color: var(--ink-soft); margin-left: 0.3125rem; }
.hero-label { font-size: 0.8125rem; color: var(--ink-soft); }

/* Countdown ring, filled by --p (0-100). */
.ring {
  flex: none;
  width: 4.875rem;
  height: 4.875rem;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: conic-gradient(
    var(--accent) calc(var(--p) * 1%),
    rgba(255, 255, 255, 0.10) 0
  );
}
.ring-inner {
  width: 3.875rem;
  height: 3.875rem;
  border-radius: 50%;
  background: rgba(31, 35, 64, 0.78);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 1px;
}
.ring-num { font-size: 1.3125rem; font-weight: 640; line-height: 1; font-variant-numeric: tabular-nums; }
.ring-cap { font-size: 0.5625rem; letter-spacing: 0.10em; text-transform: uppercase; color: var(--ink-faint); }

.row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  width: 100%;
  min-height: var(--tap);
  padding: 0.8125rem 0.75rem;
  border: 0;
  border-radius: 14px;
  background: transparent;
  color: var(--ink);
  font-size: 0.9375rem;
  text-align: left;
  cursor: pointer;
}
.row + .row { box-shadow: inset 0 1px 0 var(--line); }
.row:active { background: rgba(255, 255, 255, 0.07); }
.row-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 0.125rem; }
.row-title { font-weight: 560; }
.row-sub { font-size: 0.78rem; color: var(--ink-faint); }
.row-meta { font-size: 0.8125rem; color: var(--ink-soft); flex: none; font-variant-numeric: tabular-nums; }
.chevron { color: var(--ink-faint); flex: none; display: flex; }

.marker {
  flex: none;
  width: 2.125rem;
  height: 2.125rem;
  border-radius: 11px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(159, 192, 255, 0.14);
  color: var(--accent);
}
.marker.warm { background: rgba(242, 196, 141, 0.16); color: var(--warm); }
.marker.dim  { background: rgba(255, 255, 255, 0.07); color: var(--ink-soft); }

.stats { display: flex; gap: 0.625rem; }
.stat {
  flex: 1;
  border-radius: 14px;
  border: 1px solid var(--line);
  background: rgba(255, 255, 255, 0.05);
  padding: 0.75rem;
  display: flex;
  flex-direction: column;
  gap: 0.1875rem;
}
.stat-value { font-size: 1.1875rem; font-weight: 620; letter-spacing: -0.01em; font-variant-numeric: tabular-nums; }
.stat-label { font-size: 0.6875rem; letter-spacing: 0.06em; text-transform: uppercase; color: var(--ink-faint); }

.chart { display: flex; align-items: flex-end; gap: 0.4375rem; height: 6.5rem; }
.bar {
  flex: 1;
  border-radius: 5px 5px 3px 3px;
  background: linear-gradient(180deg, var(--accent) 0%, rgba(159, 192, 255, 0.12) 100%);
  min-height: 4px;
}
.bar.last { background: linear-gradient(180deg, #ffffff 0%, rgba(242, 196, 141, 0.28) 100%); }
.axis { display: flex; gap: 0.4375rem; }
.axis span {
  flex: 1; text-align: center; font-size: 0.625rem; color: var(--ink-faint);
  font-variant-numeric: tabular-nums;
}

/* Modelled medication level: the figures on the left, the moment they are read at on the right.
   `last baseline` where it is understood, and the bottom edge where it is not. */
.level-head {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  gap: 0.25rem 0.75rem;
  align-items: flex-end;
  align-items: last baseline;
}
.level-value {
  font-size: 1.5rem;
  font-weight: 640;
  letter-spacing: -0.02em;
  line-height: 1.1;
  font-variant-numeric: tabular-nums;
}
/* Pushed to the far edge, so it reads as the whole block's rather than as another column of the
   row it happens to sit on. */
.level-when {
  margin: 0;
  margin-left: auto;
  text-align: right;
  font-size: 0.78rem;
  color: var(--ink-soft);
  font-variant-numeric: tabular-nums;
}

/* With more than one curve the readout is also the legend: a swatch and a name against every
   figure, so which line is which is never carried by colour alone. */
.level-list { display: flex; flex-direction: column; gap: 0.375rem; margin: 0.125rem 0 0; }
.level-row { display: flex; align-items: baseline; gap: 0.5rem; }
.level-swatch {
  flex: none;
  width: 0.625rem;
  height: 0.625rem;
  border-radius: 999px;
  background: currentColor;
  transform: translateY(-0.0625rem);
}
.level-of { flex: 1; font-size: 0.875rem; color: var(--ink-soft); }
.level-at { font-size: 1.0625rem; font-weight: 620; font-variant-numeric: tabular-nums; }

/* Takes the keyboard on behalf of the drawing. No border of its own: the focus ring is the only
   thing that should say it is here. */
.plot-frame { border-radius: var(--radius); }
.plot-frame:focus { outline: none; }
.plot-frame:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
/* Shown on focus, so a phone never draws it. */
.plot-keys {
  display: none;
  margin-top: 0.5rem;
  font-size: 0.6875rem;
  color: var(--ink-faint);
}
.plot-frame:focus-visible .plot-keys { display: block; }

/* Scales with the column; the viewBox fixes its proportions. `pan-y` leaves the page its
   vertical scroll and hands plot.js everything across and every pinch. An SVG under a mouse is
   otherwise draggable as an image, and the selection that starts instead swallows every move
   after the first. */
.plot {
  display: block;
  width: 100%;
  height: auto;
  /* The chart is drawn further than it shows. The curve is clipped to the window; this is what
     keeps the date labels either side of it off the cards above and below. */
  overflow: hidden;
  touch-action: pan-y;
  user-select: none;
  -webkit-user-select: none;
  -webkit-user-drag: none;
}
/* Catches touches over the whole chart, including the empty parts. */
.plot-surface { fill: transparent; }
/* Each curve sits in a group that sets `color` to its drug's tint, so no rule here names one. */
.plot-area { fill: currentColor; opacity: 0.14; }
/* A pinch scales the layer horizontally, which would drag the stroke widths with it. */
.plot-line, .plot-ahead, .plot-dose, .plot-now, .plot-cursor { vector-effect: non-scaling-stroke; }
.plot-line, .plot-ahead {
  fill: none;
  stroke: currentColor;
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
}
/* Past now the curve is projected, not something that happened. */
.plot-ahead { stroke-dasharray: 3 4.5; opacity: 0.5; }
.plot-base { stroke: var(--line-strong); stroke-width: 1; }
.plot-rule { stroke: var(--line); stroke-width: 1; }
.plot-settles { fill: rgba(242, 196, 141, 0.10); }
.plot-now { stroke: rgba(255, 255, 255, 0.30); stroke-width: 1; }
/* The cursor belongs to no curve: it marks the moment being read, not a value. */
.plot-cursor { stroke: var(--warm); stroke-width: 1.5; }
/* Where each curve crosses it. The ring is the card's colour, so two picks that land together
   do not read as one mark. */
.plot-pick { fill: currentColor; stroke: #2b2f4f; stroke-width: 2; }
.plot-dose { stroke: currentColor; stroke-width: 1.5; stroke-linecap: round; }
.plot-label {
  fill: var(--ink-faint);
  font-size: 9px;
  font-variant-numeric: tabular-nums;
}

.step { display: flex; align-items: center; flex-wrap: wrap; gap: 0.4375rem; padding: 0.375rem 0; }
.step + .step { box-shadow: inset 0 1px 0 var(--line); }
.step-figure {
  flex: 1;
  min-width: 3.25rem;
  min-height: var(--tap);
  padding: 0.5rem 0.625rem;
  border-radius: 12px;
  border: 1px solid var(--line);
  background: rgba(22, 25, 50, 0.34);
  color: var(--ink);
  font: inherit;
  font-size: 0.9375rem;
  font-variant-numeric: tabular-nums;
}
.step-figure:focus { outline: none; border-color: rgba(159, 192, 255, 0.55); }
.step-unit { font-size: 0.78rem; color: var(--ink-faint); flex: none; }
.step-drop {
  width: var(--tap);
  height: var(--tap);
  flex: none;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 12px;
  background: transparent;
  color: var(--ink-faint);
  cursor: pointer;
}
.step-drop:active { background: rgba(255, 255, 255, 0.07); }

.chips { display: flex; flex-wrap: wrap; gap: 0.5rem; }
.chip {
  display: inline-flex;
  align-items: center;
  min-height: var(--tap);
  padding: 0 1rem;
  border-radius: 999px;
  border: 1px solid var(--line);
  background: rgba(255, 255, 255, 0.05);
  color: var(--ink-soft);
  font-size: 0.875rem;
  cursor: pointer;
}
.chip.on {
  border-color: rgba(159, 192, 255, 0.45);
  background: rgba(159, 192, 255, 0.18);
  color: var(--ink);
}

/* Two fields on one line, wrapping to their own where the column is too narrow. */
.pair { display: flex; flex-wrap: wrap; gap: 0.75rem; }
.pair .field { flex: 1; min-width: 8.5rem; }

.field { display: flex; flex-direction: column; gap: 0.4375rem; }
.field label { font-size: 0.75rem; letter-spacing: 0.06em; text-transform: uppercase; color: var(--ink-faint); }
.field input, .field select, .field textarea {
  width: 100%;
  min-height: var(--tap);
  padding: 0.8125rem 0.875rem;
  border-radius: 13px;
  border: 1px solid var(--line);
  background: rgba(22, 25, 50, 0.34);
  color: var(--ink);
  font: inherit;
  font-size: 1rem;
}
.field input:focus, .field textarea:focus {
  outline: none;
  border-color: rgba(159, 192, 255, 0.55);
}

.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-height: var(--tap);
  padding: 0.875rem 1.125rem;
  border-radius: 14px;
  border: 1px solid var(--line-strong);
  background: rgba(255, 255, 255, 0.07);
  color: var(--ink);
  font-size: 1rem;
  font-weight: 560;
  cursor: pointer;
}
.btn.block { width: 100%; }
.btn.primary {
  border-color: transparent;
  background: linear-gradient(160deg, var(--accent) 0%, var(--accent-deep) 100%);
  color: #131630;
  font-weight: 640;
  box-shadow: 0 12px 28px rgba(111, 146, 224, 0.32);
}
/* Armed for a destructive tap. */
.btn.warn,
.chip.warn {
  border-color: rgba(242, 196, 141, 0.45);
  background: rgba(242, 196, 141, 0.16);
  color: var(--warm);
}
/* Red at rest, for a delete that goes further than one record. */
.btn.danger {
  border-color: rgba(242, 146, 140, 0.42);
  color: var(--danger);
}
.btn.danger.warn {
  border-color: rgba(242, 146, 140, 0.62);
  background: rgba(242, 146, 140, 0.18);
  color: var(--danger);
}
.btn:active { transform: translateY(1px); }
.btn:disabled { opacity: 0.45; box-shadow: none; cursor: default; }
.btn:disabled:active { transform: none; }

.note { margin: 0; font-size: 0.8rem; line-height: 1.55; color: var(--ink-faint); }
/* For a note about what the app knows less well than the rest. */
.note.caution { color: var(--warm); }
.note.danger { color: var(--danger); }

/* Drawn in pseudo-elements so the button can be a full-size target without the track
   growing to match. */
.toggle {
  flex: none;
  position: relative;
  width: var(--tap);
  height: var(--tap);
  padding: 0;
  border: 0;
  background: transparent;
  cursor: pointer;
}
.toggle::before,
.toggle::after {
  content: '';
  position: absolute;
  top: 50%;
  left: 50%;
}
.toggle::before {
  width: 2.75rem;
  height: 1.625rem;
  margin-left: -1.375rem;
  transform: translateY(-50%);
  border-radius: 999px;
  border: 1px solid var(--line);
  background: rgba(255, 255, 255, 0.07);
  transition: background 220ms ease, border-color 220ms ease;
}
.toggle::after {
  width: 1.25rem;
  height: 1.25rem;
  margin-left: -1.25rem;
  transform: translateY(-50%);
  border-radius: 50%;
  background: var(--ink-faint);
  transition: transform 220ms ease, background 220ms ease;
}
.toggle.on::before { background: rgba(159, 192, 255, 0.28); border-color: rgba(159, 192, 255, 0.42); }
.toggle.on::after { transform: translate(1.25rem, -50%); background: var(--ink); }

/* Landscape and split screen: bands and hero shrink so the scrolling column keeps the
   height. */
@media (max-height: 34rem) {
  .topbar { padding-top: calc(0.5rem + env(safe-area-inset-top, 0px)); padding-bottom: 0.375rem; }
  .topbar h1 { font-size: 1.3rem; }
  .topbar .subtitle { display: none; }
  .hero-value { font-size: 1.6rem; }
  .ring { width: 3.5rem; height: 3.5rem; }
  .ring-inner { width: 2.75rem; height: 2.75rem; }
  .chart { height: 4.5rem; }
  .content { padding-bottom: calc(5.5rem + env(safe-area-inset-bottom, 0px)); }
}

/* Durations collapse to 1ms rather than 0: the shell keys its remount on the enter animation,
   and a zero duration is not guaranteed to fire the events that depend on it. */
@media (prefers-reduced-motion: reduce) {
  .drift, .twinkle { animation: none; }
  .sky-plane, .sky-haze, .sky-glow { transition-duration: 1ms; }
  .shell { animation-duration: 1ms; transition-duration: 1ms; }
  .toggle::after { transition: background 220ms ease; }
}
"#;
