//! The level chart: the arithmetic that turns days and micrograms into a drawing, and the
//! drawing itself.
//!
//! Nothing here computes a level. The caller works out the curve and hands it over already
//! sampled, so this module is only ever about where a figure lands on the page.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeDelta};
use dioxus::prelude::*;
use serde::Deserialize;

use crate::formulary::Drug;
use crate::kinetics::Sample;

/// Several drugs share an axis of milligrams but never a line, since a milligram of one drug is
/// not a milligram of another.
#[derive(Clone, PartialEq, Debug)]
pub struct Series {
    pub drug: Drug,
    /// Sampled across the window and some way either side of it, as [`LevelPlot`] describes.
    pub samples: Vec<Sample>,
    /// Days a dose of this drug sits on, ticked under the baseline.
    pub marks: Vec<f64>,
    /// The level at the cursor, dotted where the cursor crosses this curve.
    pub pick: f64,
}

/// Pan, pinch and tap over the chart.
///
/// It transforms the drawing under the finger and reports here once, when the gesture ends: a
/// message per touch event would cross the `WebView`'s IPC channel faster than it carries.
pub const PLOT: &str = include_str!("plot.js");

/// The window a gesture left behind.
#[derive(Deserialize)]
pub struct Moved {
    pub from: f64,
    pub to: f64,
    /// The day a tap landed on, where the gesture was a tap rather than a drag.
    pub read: Option<f64>,
}

/// The same reach the gestures have, for anyone not making them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Nudge {
    In,
    Out,
    Back,
    Forward,
    Now,
}

/// What a keyed zoom multiplies or divides the span by.
const ZOOM_BY: f64 = 2.0;
/// How much of the window a keyed pan crosses.
const PAN_BY: f64 = 0.25;

impl Nudge {
    /// `=` and `_` are the unshifted faces of the `+` and `-` keys, which is what a keyboard
    /// reports without shift.
    pub fn of(key: &Key) -> Option<Self> {
        match key {
            Key::ArrowLeft => Some(Self::Back),
            Key::ArrowRight => Some(Self::Forward),
            Key::Home => Some(Self::Now),
            Key::Character(pressed) => match pressed.as_str() {
                "+" | "=" => Some(Self::In),
                "-" | "_" => Some(Self::Out),
                _ => None,
            },
            _ => None,
        }
    }

    /// The window this leaves, from the one it was made over. Unfitted: [`fit`] runs on every
    /// path into the window, so this only says where to aim.
    pub fn from(self, (from, to): (f64, f64), now: f64) -> (f64, f64) {
        let span = to - from;
        let middle = f64::midpoint(from, to);
        let across = |span: f64| (middle - span / 2.0, middle + span / 2.0);
        match self {
            Self::In => across(span / ZOOM_BY),
            Self::Out => across(span * ZOOM_BY),
            Self::Back => (from - span * PAN_BY, to - span * PAN_BY),
            Self::Forward => (from + span * PAN_BY, to + span * PAN_BY),
            Self::Now => (now - span / 2.0, now + span / 2.0),
        }
    }
}

/// Room on the left for the value labels.
const PAD_LEFT: f64 = 30.0;
/// Room on the right, so the last date label sits inside the box.
const PAD_RIGHT: f64 = 10.0;
/// Room above, so the tallest peak is not clipped.
const PAD_TOP: f64 = 10.0;
/// Room below the baseline for the dose marks and the date labels.
const PAD_BOTTOM: f64 = 22.0;

/// The steps an axis labels at and a reading lands on, in days.
///
/// Fractions of a day because the model works in them: a dose is placed at the hour it was given
/// and the curve climbs over the hours after it.
const STEPS: [f64; 15] = [
    HOUR / 4.0,
    HOUR / 2.0,
    HOUR,
    2.0 * HOUR,
    3.0 * HOUR,
    6.0 * HOUR,
    12.0 * HOUR,
    1.0,
    2.0,
    7.0,
    14.0,
    28.0,
    91.0,
    182.0,
    364.0,
];

/// An hour, as the axis counts.
const HOUR: f64 = 1.0 / 24.0;

/// Divisions a reading is taken at, which is as fine as a finger can be trusted to have meant.
const READINGS: f64 = 64.0;

/// The narrowest window a pinch can close to: a quarter of a day, which is about as much of a
/// dose's climb as is worth looking at on its own.
pub const MIN_SPAN: f64 = 6.0 * HOUR;
/// The widest it can open to.
pub const MAX_SPAN: f64 = 1460.0;

/// The shortest step on the ladder that divides `span` no more than `target` times.
fn step_for(span: f64, target: f64) -> f64 {
    let span = if span.is_finite() { span.max(0.0) } else { 0.0 };
    STEPS
        .into_iter()
        .find(|step| span / step <= target.max(1.0))
        .unwrap_or(364.0)
}

/// How fine a reading off the chart is worth taking at this span, in days, so that it lands on a
/// figure worth quoting rather than wherever the finger fell. Never coarser than a day.
pub fn grain(span: f64) -> f64 {
    step_for(span, READINGS).min(1.0)
}

pub fn snapped(day: f64, grain: f64) -> f64 {
    if grain > 0.0 && day.is_finite() {
        (day / grain).round() * grain
    } else {
        day
    }
}

/// Fits a proposed window inside the timeline it moves along.
///
/// The span is kept where there is room for it and the window slid to make it fit, so a pan into
/// the end of the timeline stops rather than shrinking the view under the finger. `plot.js`
/// clamps the same way while a gesture is running; this is the side that decides.
pub fn fit((from, to): (f64, f64), (floor, ceiling): (f64, f64)) -> (f64, f64) {
    let room = (ceiling - floor).max(MIN_SPAN);
    let span = (to - from).clamp(MIN_SPAN.min(room), MAX_SPAN.min(room));
    let mut start = f64::midpoint(from, to) - span / 2.0;
    if start + span > ceiling {
        start = ceiling - span;
    }
    if start < floor {
        start = floor;
    }
    (start, start + span)
}

/// About `target` divisions across `span`, landing on a round figure.
///
/// The 1, 2, 5 ladder, so the chart is ruled at figures that can be read off rather than at
/// whatever an even division of the data happens to come to. The even division is rounded to the
/// nearest rung rather than up to the next: rounding up alone lands as low as half the divisions
/// asked for, which on a chart this size is two gridlines where there should be four.
pub fn nice_step(span: f64, target: f64) -> f64 {
    if !span.is_finite() || span <= 0.0 || target <= 0.0 {
        return 1.0;
    }
    let rough = span / target;
    let magnitude = 10.0_f64.powf(rough.log10().floor());
    let step = match rough / magnitude {
        scaled if scaled < 1.5 => 1.0,
        scaled if scaled < 3.0 => 2.0,
        scaled if scaled < 7.0 => 5.0,
        _ => 10.0,
    };
    step * magnitude
}

pub fn value_ticks(top: f64, target: f64) -> Vec<f64> {
    let step = nice_step(top, target);
    let mut ticks = Vec::new();
    let mut value = 0.0;
    while value <= top * 1.0001 && ticks.len() < 32 {
        ticks.push(value);
        value += step;
    }
    ticks
}

#[derive(Clone, PartialEq, Debug)]
pub struct DateAxis {
    /// Days between one label and the next, a fraction of one on a short window.
    pub step: f64,
    /// Where the labels fall, in days from the origin.
    pub at: Vec<f64>,
}

impl DateAxis {
    /// Labels every `step` days, the step being the shortest on the ladder that keeps the count
    /// near `target`. Anchored on the origin, so labels hold still as the window moves over them.
    ///
    /// The step is chosen for the window on show; the labels are laid across the wider reach,
    /// since a gesture slides them before this side hears anything.
    pub fn across((from, to): (f64, f64), (opens, closes): (f64, f64), target: f64) -> Self {
        let step = step_for(to - from, target);

        let mut at = Vec::new();
        let mut day = (opens / step).ceil() * step;
        while day <= closes && at.len() < 64 {
            at.push(day);
            day += step;
        }
        Self { step, at }
    }

    pub fn format(&self) -> &'static str {
        if self.step < 1.0 {
            "%H:%M"
        } else if self.step < 28.0 {
            "%-d %b"
        } else {
            "%b %y"
        }
    }
}

/// The furthest from the origin a point on the axis may sit and still be a date. Past this a
/// dose was dated by a typo rather than by a calendar.
const CALENDAR_REACH: f64 = 3_000_000.0;

/// `None` past [`CALENDAR_REACH`].
pub fn whole_day(day: f64) -> Option<f64> {
    let rounded = day.round();
    (rounded.is_finite() && rounded.abs() <= CALENDAR_REACH).then_some(rounded)
}

/// The moment a point on the axis falls on, to the second, or `None` past [`CALENDAR_REACH`].
///
/// Day zero is midnight at the start of `origin` and the fraction is how far through the day it
/// falls — the axis [`crate::kinetics::instant_of`] places a dose on.
pub fn instant_at(day: f64, origin: NaiveDate) -> Option<NaiveDateTime> {
    if !day.is_finite() || day.abs() > CALENDAR_REACH {
        return None;
    }
    // Bounded just above, so nothing is lost in the conversion.
    #[allow(clippy::cast_possible_truncation)]
    let seconds = (day * 86_400.0).round() as i64;
    origin
        .and_time(NaiveTime::MIN)
        .checked_add_signed(TimeDelta::seconds(seconds))
}

/// Maps days and micrograms onto the chart's box.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Scale {
    from: f64,
    to: f64,
    top: f64,
    width: f64,
    height: f64,
}

impl Scale {
    /// The scale for a window, or `None` for one with no width or nothing to show in it.
    pub fn new(from: f64, to: f64, top: f64, width: f64, height: f64) -> Option<Self> {
        (to > from && top > 0.0 && width > PAD_LEFT + PAD_RIGHT && height > PAD_TOP + PAD_BOTTOM)
            .then_some(Self {
                from,
                to,
                top,
                width,
                height,
            })
    }

    pub fn from(self) -> f64 {
        self.from
    }

    pub fn to(self) -> f64 {
        self.to
    }

    pub fn top(self) -> f64 {
        self.top
    }

    pub fn width(self) -> f64 {
        self.width
    }

    pub fn height(self) -> f64 {
        self.height
    }

    /// Where zero sits.
    pub fn base(self) -> f64 {
        self.height - PAD_BOTTOM
    }

    fn plotted(self) -> f64 {
        self.width - PAD_LEFT - PAD_RIGHT
    }

    /// Carries a day outside the window past the edge of the box rather than clamping it, so the
    /// overscan lands where a gesture can bring it in.
    pub fn x(self, day: f64) -> f64 {
        PAD_LEFT + (day - self.from) / (self.to - self.from) * self.plotted()
    }

    pub fn y(self, micrograms: f64) -> f64 {
        self.base() - (micrograms / self.top) * (self.base() - PAD_TOP)
    }
}

/// One series as the three paths that draw it.
struct Run {
    /// What was logged, solid.
    line: String,
    /// The same, closed to the baseline. Only under a lone curve: two washes over each other read
    /// as a third colour where they cross.
    area: Option<String>,
    /// What the plan projects, dashed.
    projected: String,
}

impl Run {
    /// The paths one series is drawn as. `wash` asks for the fill as well, which is a path
    /// through every sample again.
    fn of(series: &Series, now: f64, scale: Scale, wash: bool) -> Self {
        // The two runs share the sample either side of now, so the solid line and the dashed one
        // meet rather than leaving a gap.
        let split = series
            .samples
            .iter()
            .position(|sample| sample.day > now)
            .unwrap_or(series.samples.len());
        let behind = &series.samples[..split];
        let ahead = &series.samples[split.saturating_sub(1)..];

        let line = path_through(behind, scale);
        let area = behind
            .first()
            .zip(behind.last())
            .filter(|_| wash)
            .map(|(opening, closing)| {
                format!(
                    "{line} L {:.1} {base:.1} L {:.1} {base:.1} Z",
                    scale.x(closing.day),
                    scale.x(opening.day),
                    base = scale.base()
                )
            });
        Self {
            line,
            area,
            projected: path_through(ahead, scale),
        }
    }
}

/// An SVG path through samples, `M` on the first and `L` on the rest.
fn path_through(samples: &[Sample], scale: Scale) -> String {
    samples
        .iter()
        .enumerate()
        .map(|(index, sample)| {
            let command = if index == 0 { 'M' } else { 'L' };
            format!(
                "{command} {:.1} {:.1}",
                scale.x(sample.day),
                scale.y(sample.micrograms)
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Milligrams as an axis label, at as few decimals as tell one tick from the next.
fn tick_label(micrograms: f64, step: f64) -> String {
    let mg = micrograms / 1000.0;
    if step >= 1000.0 {
        format!("{mg:.0}")
    } else if step >= 100.0 {
        format!("{mg:.1}")
    } else {
        format!("{mg:.2}")
    }
}

/// The level chart. Each line runs solid to `now` and dashed past it, so what was logged is told
/// apart from what the plan projects.
#[component]
#[allow(clippy::needless_pass_by_value)]
pub fn LevelPlot(
    /// Sampled across the window and some way either side: a gesture slides the drawing before
    /// this side hears anything, so what it brings in from an edge has to be here already.
    series: Vec<Series>,
    origin: NaiveDate,
    scale: Scale,
    now: f64,
    /// The timeline the window moves along, which a gesture cannot take it past.
    bounds: (f64, f64),
    /// Where the current strength settles, as trough and peak, banded across the chart. Only ever
    /// set for a lone curve: two bands over each other say less than either would alone.
    settles: Option<(f64, f64)>,
    /// The point on the axis being read off. Each curve is dotted where it crosses.
    cursor: Option<f64>,
    onnudge: EventHandler<Nudge>,
) -> Element {
    let alone = series.len() == 1;
    let runs: Vec<Run> = series
        .iter()
        .map(|drawn| Run::of(drawn, now, scale, alone))
        .collect();

    // How far the drawing reaches, which is as far as the caller sampled. `plot.js` may move the
    // window anywhere inside this and no further. The shortest reach decides: past it one of the
    // curves would run out.
    let drawn = series
        .iter()
        .filter_map(|drawn| drawn.samples.first().zip(drawn.samples.last()))
        .fold(None, |reach: Option<(f64, f64)>, (first, last)| {
            Some(reach.map_or((first.day, last.day), |(opens, closes)| {
                (opens.max(first.day), closes.min(last.day))
            }))
        })
        .unwrap_or((scale.from(), scale.to()));
    let shows = |day: f64| day >= drawn.0 && day <= drawn.1;

    let step = nice_step(scale.top(), 4.0);
    let dates = DateAxis::across((scale.from(), scale.to()), drawn, 4.0);
    let format = dates.format();
    let right = scale.width() - PAD_RIGHT;

    rsx! {
        // The frame takes focus and answers keys, and carries the label: the SVG inside is
        // `aria-hidden`, and a focusable element inside a hidden one is incoherent to a reader.
        div {
            class: "plot-frame",
            tabindex: "0",
            role: "group",
            aria_label: "Medication level over time. Plus and minus zoom, left and right move through time, Home returns to now.",
            onkeydown: move |event| {
                if let Some(nudge) = Nudge::of(&event.key()) {
                    // Or the arrows scroll the page out from under the chart.
                    event.prevent_default();
                    onnudge.call(nudge);
                }
            },

            svg {
                class: "plot",
                view_box: "0 0 {scale.width()} {scale.height()}",
                // Hidden from readers, so the caller has to state the figures in text beside it.
                "aria-hidden": "true",
                // `plot.js` reads the whole of its geometry off these rather than knowing any of
                // it. Every marker carries a value: an attribute set to nothing is not reliably
                // rendered as one, and a selector for it would then match nothing.
                "data-plot": "level",
                "data-swipe": "off",
                "data-from": "{scale.from()}",
                "data-to": "{scale.to()}",
                "data-floor": "{bounds.0}",
                "data-ceiling": "{bounds.1}",
                "data-drawn-from": "{drawn.0}",
                "data-drawn-to": "{drawn.1}",
                "data-min-span": "{MIN_SPAN}",
                "data-max-span": "{MAX_SPAN}",
                "data-view-width": "{scale.width()}",
                "data-pad-left": "{PAD_LEFT}",
                "data-pad-right": "{PAD_RIGHT}",

                // The window, as a shape to cut the moving layer down to. The curve runs past both
                // edges of it and above the top of the axis, and none of that may be seen until a
                // gesture brings it in.
                // The id is fixed, and an SVG fragment reference is document-wide: a second chart
                // on the page would define the same one and either could win.
                defs {
                    clipPath { id: "atlas-plot-window",
                        rect {
                            x: "{PAD_LEFT:.1}",
                            y: "0",
                            width: "{scale.width() - PAD_LEFT - PAD_RIGHT:.1}",
                            height: "{scale.height():.1}",
                        }
                    }
                }

                // An SVG only answers a touch where it is painted, and a chart is mostly not, so
                // a drag between the curve and a rule would otherwise land on the page. A
                // transparent fill still counts as painted.
                rect {
                    class: "plot-surface",
                    x: "0",
                    y: "0",
                    width: "{scale.width()}",
                    height: "{scale.height()}",
                }

                for value in value_ticks(scale.top(), 4.0) {
                    g { key: "{value}",
                        line {
                            class: if value > 0.0 { "plot-rule" } else { "plot-base" },
                            x1: "{PAD_LEFT:.1}",
                            y1: "{scale.y(value):.1}",
                            x2: "{right:.1}",
                            y2: "{scale.y(value):.1}",
                        }
                        text {
                            class: "plot-label",
                            x: "{PAD_LEFT - 5.0:.1}",
                            y: "{scale.y(value) + 3.0:.1}",
                            text_anchor: "end",
                            "{tick_label(value, step)}"
                        }
                    }
                }

                if let Some((trough, peak)) = settles {
                    rect {
                        class: "plot-settles",
                        x: "{PAD_LEFT:.1}",
                        y: "{scale.y(peak):.1}",
                        width: "{scale.width() - PAD_LEFT - PAD_RIGHT:.1}",
                        height: "{(scale.y(trough) - scale.y(peak)).max(0.0):.1}",
                    }
                }

                // The clip stays put while the layer inside it moves: a clip path on the layer
                // itself would be carried along by the same transform and cut nothing.
                g { "clip-path": "url(#atlas-plot-window)",
                    // The value rules stay outside it: a pan and a pinch are horizontal.
                    g { "data-plot-layer": "days",
                        // A group per drug, carrying its tint as `color`; everything inside is
                        // painted in `currentColor`.
                        for (drawn , run) in series.iter().zip(&runs) {
                            g {
                                key: "{drawn.drug:?}",
                                style: "color: var({drawn.drug.tint()})",
                                if let Some(area) = &run.area {
                                    path { class: "plot-area", d: "{area}" }
                                }
                                path { class: "plot-ahead", d: "{run.projected}" }
                                path { class: "plot-line", d: "{run.line}" }

                                for mark in &drawn.marks {
                                    line {
                                        key: "{mark}",
                                        class: "plot-dose",
                                        x1: "{scale.x(*mark):.1}",
                                        y1: "{scale.base() + 2.0:.1}",
                                        x2: "{scale.x(*mark):.1}",
                                        y2: "{scale.base() + 7.0:.1}",
                                    }
                                }

                                if let Some(day) = cursor.filter(|day| shows(*day)) {
                                    circle {
                                        class: "plot-pick",
                                        cx: "{scale.x(day):.1}",
                                        cy: "{scale.y(drawn.pick):.1}",
                                        r: "3.4",
                                    }
                                }
                            }
                        }

                        // Panned far enough away, now is off the drawing and has no line to draw.
                        if shows(now) {
                            line {
                                class: "plot-now",
                                x1: "{scale.x(now):.1}",
                                y1: "{PAD_TOP:.1}",
                                x2: "{scale.x(now):.1}",
                                y2: "{scale.base():.1}",
                            }
                        }

                        // One rule for every curve: it marks the day being read, not a value.
                        if let Some(day) = cursor.filter(|day| shows(*day)) {
                            line {
                                class: "plot-cursor",
                                x1: "{scale.x(day):.1}",
                                y1: "{PAD_TOP:.1}",
                                x2: "{scale.x(day):.1}",
                                y2: "{scale.base():.1}",
                            }
                        }
                    }
                }

                // Outside the moving layer, and carried by `plot.js` a label at a time: scaling
                // the group would stretch the lettering, and translating it would only hold under
                // a pan. Each label keeps the `x` it was written at, which the script moves it
                // from. Nothing clips them but the edges of the chart.
                g { "data-plot-dates": "days",
                    for day in dates.at {
                        if let Some(date) = instant_at(day, origin) {
                            text {
                                key: "{day}",
                                class: "plot-label",
                                x: "{scale.x(day):.1}",
                                y: "{scale.height() - 5.0:.1}",
                                text_anchor: "middle",
                                "{date.format(format)}"
                            }
                        }
                    }
                }
            }

            // Shown only on focus, so a phone never draws a line about keys it has none of.
            p { class: "plot-keys",
                "+ and − zoom · ← and → move through time · Home returns to now"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").expect("a date the test wrote")
    }

    fn on_step(day: f64, step: f64) -> bool {
        let off = day / step;
        (off - off.round()).abs() < 1e-6
    }

    #[test]
    fn a_step_is_a_figure_a_reader_would_count_in() {
        for (span, target, expected) in [
            (10.0, 5.0, 2.0),
            (100.0, 5.0, 20.0),
            (1.0, 5.0, 0.2),
            (4500.0, 4.0, 1000.0),
            (0.9, 2.0, 0.5),
            (4830.0, 4.0, 1000.0),
        ] {
            let step = nice_step(span, target);
            assert!(
                (step - expected).abs() < 1e-9,
                "{span} over {target} stepped by {step}, not {expected}"
            );
        }
    }

    #[test]
    fn a_step_survives_a_span_it_cannot_divide() {
        assert!(nice_step(0.0, 4.0) > 0.0);
        assert!(nice_step(-5.0, 4.0) > 0.0);
        assert!(nice_step(f64::NAN, 4.0) > 0.0);
        assert!(nice_step(10.0, 0.0) > 0.0);
    }

    #[test]
    fn ticks_start_at_zero_and_stop_at_the_top() {
        let ticks = value_ticks(4200.0, 4.0);

        assert!((ticks[0] - 0.0).abs() < 1e-9, "the axis starts at nothing");
        assert!(ticks.len() >= 3, "{ticks:?} is too few to read against");
        assert!(*ticks.last().expect("at least one") <= 4200.0);
        for pair in ticks.windows(2) {
            assert!(pair[1] > pair[0], "ticks climb");
        }
    }

    #[test]
    fn a_date_axis_takes_a_wider_step_the_longer_the_span() {
        let fortnight = DateAxis::across((0.0, 14.0), (0.0, 14.0), 4.0);
        let month = DateAxis::across((0.0, 30.0), (0.0, 30.0), 4.0);
        let year = DateAxis::across((0.0, 364.0), (0.0, 364.0), 4.0);

        assert!(fortnight.step <= 7.0, "{} is too coarse", fortnight.step);
        assert!(month.step <= 14.0);
        assert!(year.step >= 91.0);
        assert_eq!(fortnight.format(), "%-d %b");
        assert_eq!(year.format(), "%b %y");
    }

    #[test]
    fn a_date_axis_labels_in_hours_over_a_window_shorter_than_a_day() {
        let hours = DateAxis::across((0.0, 0.25), (0.0, 0.25), 4.0);

        assert!(hours.step < 1.0, "{} is a day or more", hours.step);
        assert!(
            hours.step >= HOUR / 4.0,
            "{} is finer than the ladder",
            hours.step
        );
        assert!(
            hours.at.len() >= 2,
            "{:?} is too few to read against",
            hours.at
        );
        assert_eq!(hours.format(), "%H:%M");
    }

    #[test]
    fn a_date_axis_labels_inside_its_window_and_nowhere_else() {
        let axis = DateAxis::across((13.0, 71.0), (13.0, 71.0), 4.0);

        assert!(!axis.at.is_empty());
        for day in &axis.at {
            assert!((13.0..=71.0).contains(day), "{day} is outside the window");
            assert!(on_step(*day, axis.step), "{day} is off the step");
        }
    }

    #[test]
    fn a_date_axis_labels_past_the_window_at_the_step_the_window_asked_for() {
        let window = DateAxis::across((30.0, 60.0), (30.0, 60.0), 4.0);
        let reaching = DateAxis::across((30.0, 60.0), (0.0, 90.0), 4.0);

        assert!(
            (reaching.step - window.step).abs() < 1e-12,
            "the step follows the window"
        );
        assert!(
            reaching.at.len() > window.at.len(),
            "{:?} reaches no further than {:?}",
            reaching.at,
            window.at
        );
        assert!(reaching.at.iter().any(|day| *day < 30.0), "none before it");
        assert!(reaching.at.iter().any(|day| *day > 60.0), "none after it");
        for day in &window.at {
            assert!(reaching.at.contains(day), "{day} was dropped");
        }
    }

    #[test]
    fn a_date_axis_holds_still_as_a_window_moves_over_it() {
        let earlier = DateAxis::across((0.0, 60.0), (0.0, 60.0), 4.0);
        let later = DateAxis::across((10.0, 70.0), (10.0, 70.0), 4.0);

        assert!(
            (earlier.step - later.step).abs() < 1e-12,
            "the step does not change"
        );
        for day in &later.at {
            assert!(
                on_step(*day, later.step),
                "labels stay on the same days, not the window's edge"
            );
        }
    }

    #[test]
    fn a_reading_is_taken_as_finely_as_the_zoom_can_be_aimed() {
        let close = grain(MIN_SPAN);
        let day = grain(1.0);
        let month = grain(30.0);
        let year = grain(364.0);

        assert!(close < HOUR, "{close} days is no finer than an hour");
        assert!(day <= HOUR, "{day} days is too coarse to read a day at");
        assert!(
            month > HOUR && month < 1.0,
            "{month} days is not an hour or so"
        );
        assert!(
            (year - 1.0).abs() < 1e-12,
            "a year reads by the day and no finer"
        );
        assert!(
            grain(f64::NAN) > 0.0 && grain(-5.0) > 0.0,
            "and never nothing"
        );
    }

    #[test]
    fn a_reading_lands_on_the_grain_it_was_taken_at() {
        assert!((snapped(3.44, 1.0) - 3.0).abs() < 1e-9);
        assert!((snapped(3.51, 1.0) - 4.0).abs() < 1e-9);
        // 08:20 read at an hour's grain is 08:00.
        assert!((snapped(3.0 + 8.34 * HOUR, HOUR) - (3.0 + 8.0 * HOUR)).abs() < 1e-9);
        assert!((snapped(-0.4, 1.0) - 0.0).abs() < 1e-9);
        assert!(
            (snapped(2.5, 0.0) - 2.5).abs() < 1e-9,
            "no grain leaves it alone"
        );
    }

    #[test]
    fn a_point_between_two_days_is_a_time_on_the_calendar() {
        let origin = date("2026-08-06");

        assert_eq!(
            instant_at(0.0, origin),
            Some(origin.and_hms_opt(0, 0, 0).expect("midnight"))
        );
        assert_eq!(
            instant_at(1.5, origin),
            Some(date("2026-08-07").and_hms_opt(12, 0, 0).expect("midday"))
        );
        assert_eq!(
            instant_at(-0.25, origin),
            Some(
                date("2026-08-05")
                    .and_hms_opt(18, 0, 0)
                    .expect("the evening before")
            )
        );
        assert_eq!(instant_at(f64::INFINITY, origin), None);
        assert_eq!(instant_at(1e9, origin), None);
    }

    #[test]
    fn a_date_axis_over_a_window_with_no_width_still_ends() {
        assert!(DateAxis::across((5.0, 5.0), (5.0, 5.0), 4.0).at.len() <= 1);
    }

    #[test]
    fn a_scale_needs_a_box_with_room_in_it() {
        assert!(Scale::new(0.0, 7.0, 4200.0, 320.0, 150.0).is_some());
        assert_eq!(Scale::new(7.0, 7.0, 4200.0, 320.0, 150.0), None, "no span");
        assert_eq!(
            Scale::new(0.0, 7.0, 0.0, 320.0, 150.0),
            None,
            "nothing tall"
        );
        assert_eq!(Scale::new(0.0, 7.0, 4200.0, 8.0, 150.0), None, "too narrow");
        assert_eq!(Scale::new(0.0, 7.0, 4200.0, 320.0, 4.0), None, "too short");
    }

    #[test]
    fn a_scale_puts_the_window_against_the_edges_of_its_box() {
        let scale = Scale::new(3.0, 17.0, 4200.0, 320.0, 150.0).expect("a box with room");

        assert!((scale.x(3.0) - PAD_LEFT).abs() < 1e-9);
        assert!((scale.x(17.0) - (320.0 - PAD_RIGHT)).abs() < 1e-9);
        assert!((scale.y(0.0) - scale.base()).abs() < 1e-9);
        assert!((scale.y(4200.0) - PAD_TOP).abs() < 1e-9);
        assert!(scale.y(4200.0) < scale.y(0.0), "more drug reads higher");
    }

    #[test]
    fn a_key_moves_the_window_the_way_a_gesture_would() {
        let window = (10.0, 40.0);

        let (from, to) = Nudge::In.from(window, 0.0);
        assert!(to - from < 30.0, "{from}..{to} did not close");
        assert!(
            (f64::midpoint(from, to) - 25.0).abs() < 1e-9,
            "a zoom holds the middle of the window still"
        );

        let (from, to) = Nudge::Out.from(window, 0.0);
        assert!(to - from > 30.0, "{from}..{to} did not open");

        let (from, to) = Nudge::Back.from(window, 0.0);
        assert!(
            from < 10.0 && (to - from - 30.0).abs() < 1e-9,
            "it kept its span"
        );
        let (from, to) = Nudge::Forward.from(window, 0.0);
        assert!(from > 10.0 && (to - from - 30.0).abs() < 1e-9);

        let (from, to) = Nudge::Now.from(window, 500.0);
        assert!((f64::midpoint(from, to) - 500.0).abs() < 1e-9);
        assert!(
            (to - from - 30.0).abs() < 1e-9,
            "returning to now holds the zoom"
        );
    }

    #[test]
    fn only_the_keys_the_chart_answers_are_taken() {
        assert_eq!(Nudge::of(&Key::ArrowLeft), Some(Nudge::Back));
        assert_eq!(Nudge::of(&Key::ArrowRight), Some(Nudge::Forward));
        assert_eq!(Nudge::of(&Key::Home), Some(Nudge::Now));
        assert_eq!(Nudge::of(&Key::Character("=".into())), Some(Nudge::In));
        assert_eq!(Nudge::of(&Key::Character("-".into())), Some(Nudge::Out));
        assert_eq!(Nudge::of(&Key::ArrowDown), None);
        assert_eq!(Nudge::of(&Key::Character("k".into())), None);
    }

    #[test]
    fn a_window_inside_the_timeline_is_left_where_the_gesture_put_it() {
        assert_eq!(fit((10.0, 40.0), (0.0, 100.0)), (10.0, 40.0));
    }

    #[test]
    fn a_window_panned_past_an_end_stops_against_it_at_the_span_it_had() {
        let (from, to) = fit((80.0, 110.0), (0.0, 100.0));
        assert_eq!((from, to), (70.0, 100.0));
        assert!((to - from - 30.0).abs() < 1e-9, "it kept its span");

        let (from, to) = fit((-20.0, 10.0), (0.0, 100.0));
        assert_eq!((from, to), (0.0, 30.0));
    }

    #[test]
    fn a_window_wider_than_the_timeline_settles_on_the_whole_of_it() {
        assert_eq!(fit((-500.0, 500.0), (0.0, 100.0)), (0.0, 100.0));
    }

    #[test]
    fn a_pinch_cannot_close_past_the_narrowest_window_or_open_past_the_widest() {
        let (from, to) = fit((50.0, 50.2), (0.0, 4000.0));
        assert!(
            (to - from - MIN_SPAN).abs() < 1e-9,
            "closed to {}",
            to - from
        );

        let (from, to) = fit((0.0, 3000.0), (0.0, 4000.0));
        assert!(
            (to - from - MAX_SPAN).abs() < 1e-9,
            "opened to {}",
            to - from
        );
    }

    #[test]
    fn a_timeline_shorter_than_the_narrowest_window_is_still_a_window() {
        let (from, to) = fit((0.0, 1.0), (0.0, 1.0));
        assert!(to > from, "a scale can be built on {from}..{to}");
        assert!(Scale::new(from, to, 4200.0, 320.0, 150.0).is_some());
    }

    #[test]
    fn a_day_outside_the_window_maps_outside_the_box() {
        let scale = Scale::new(10.0, 20.0, 4200.0, 320.0, 150.0).expect("a box with room");

        assert!(scale.x(25.0) > scale.width() - PAD_RIGHT);
        assert!(scale.x(5.0) < PAD_LEFT);
    }

    #[test]
    fn a_day_on_the_axis_is_a_date_on_the_calendar() {
        let origin = date("2026-08-06");
        let on = |day: f64| instant_at(day, origin).map(|at| at.date());

        assert_eq!(on(0.0), Some(origin));
        assert_eq!(on(7.0), Some(date("2026-08-13")));
        assert_eq!(
            on(7.5),
            Some(date("2026-08-13")),
            "still that day at midday"
        );
        assert_eq!(on(-6.0), Some(date("2026-07-31")));
    }

    #[test]
    fn a_day_past_the_calendar_is_no_day_at_all() {
        assert_eq!(whole_day(f64::INFINITY), None);
        assert_eq!(whole_day(1e9), None);
        assert_eq!(whole_day(6.6), Some(7.0));
    }
}
