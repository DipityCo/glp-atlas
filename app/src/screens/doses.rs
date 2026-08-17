//! The Doses page and the pages reached from it.
//!
//! Everything here reads and writes the one dose log in [`crate::store`]; nothing on these
//! screens is a fixed figure.

use std::cmp::Ordering;

use chrono::NaiveDate;
use dioxus::document;
use dioxus::prelude::*;

use super::inventory::{vial_label, MixControl};
use super::Row;
use crate::chart::{self, Moved, PLOT};
use crate::formulary::Drug;
use crate::icons::{CalendarClock, Pill, Syringe, Vial as VialIcon};
use crate::kinetics::{curve, instant_of, moment, on_board, spent, Given, Kinetics, Sample, TRACE};
use crate::nav::{Nav, SubPage};
use crate::stock::{Stock, VialId};
use crate::store::{
    format_time, parse_time, rung, suggest_site, time_of_day, time_value, today, Clock, Cycle,
    Dose, DoseId, Draft, Medication, Site, Status, Store, TitrationStep,
};
use crate::units::{format_level, format_mg, format_units, format_volume, parse_mg};

/// Weekday, day and month: `Thursday 6 Aug`.
fn long_date(date: NaiveDate) -> String {
    date.format("%A %-d %b").to_string()
}

/// Day and month: `6 Aug`. For the chart's readout, which has no room for a weekday.
fn short_date(date: NaiveDate) -> String {
    date.format("%-d %b").to_string()
}

/// A date as it reads in a list: named for the last two days, dated before that.
fn day_label(date: NaiveDate, today: NaiveDate) -> String {
    match (today - date).num_days() {
        0 => "Today".to_owned(),
        1 => "Yesterday".to_owned(),
        _ => long_date(date),
    }
}

/// Whether a vial holding `held` can be the source of a dose given as `given`. A vial the
/// formulary does not name could be anything, and a dose naming no drug contradicts nothing.
fn fits(held: Option<Drug>, given: Option<Drug>) -> bool {
    match (held, given) {
        (Some(held), Some(given)) => held == given,
        _ => true,
    }
}

fn strength_change(from: u32, to: u32) -> String {
    match to.cmp(&from) {
        Ordering::Greater => format!("+{} mg", format_mg(to - from)),
        Ordering::Less => format!("−{} mg", format_mg(from - to)),
        Ordering::Equal => "Same".to_owned(),
    }
}

#[component]
fn MissingDose() -> Element {
    rsx! {
        div { class: "card",
            h2 { class: "card-title", "Dose not found" }
            p { class: "note", "This dose is no longer in the log." }
        }
    }
}

#[component]
fn CycleCard(last: Dose, today: NaiveDate, interval: u32) -> Element {
    let strength = format_mg(last.micrograms);
    let Some(cycle) = Cycle::new(last.taken, today, interval) else {
        return rsx! {
            div { class: "card hero",
                div { class: "hero-main",
                    span { class: "hero-value",
                        "{strength}"
                        span { class: "hero-unit", "mg" }
                    }
                    span { class: "hero-label", "Logged {long_date(last.taken)}" }
                }
            }
        };
    };
    let (count, caption) = cycle.ring();
    let label = match cycle.remaining {
        0 => "Next dose today".to_owned(),
        days if days < 0 => format!("Was due {}", long_date(cycle.due)),
        _ => format!("Next dose {}", long_date(cycle.due)),
    };
    rsx! {
        div { class: "card hero",
            div { class: "ring", style: "--p:{cycle.fill()}",
                div { class: "ring-inner",
                    span { class: "ring-num", "{count}" }
                    span { class: "ring-cap", "{caption}" }
                }
            }
            div { class: "hero-main",
                span { class: "hero-value",
                    "{strength}"
                    span { class: "hero-unit", "mg" }
                }
                span { class: "hero-label", "{label}" }
            }
        }
    }
}

/// The chart's canvas, in the units its `viewBox` declares. The stylesheet stretches it to the
/// column, so these are proportions rather than pixels.
const CHART_WIDTH: f64 = 320.0;
const CHART_HEIGHT: f64 = 150.0;
/// Points along the curve, per window's width. Enough that a peak is not visibly faceted at the
/// closest zoom.
const RESOLUTION: u32 = 240;
/// Windows either side of the one on show that the curve is drawn across.
///
/// `plot.js` moves the drawing under the finger and only reports on release, so whatever a
/// gesture brings in from an edge has to have been drawn before it began. One window each side
/// covers a drag across the full width of the chart and a threefold pinch; past that the curve
/// runs out until the finger lifts.
const OVERSCAN: u32 = 1;
/// Cycles a plan is carried forward for. Far enough to show a titration step arriving.
const PROJECTED_CYCLES: u32 = 12;

/// How much of the timeline the chart opens on: a month, which holds four weekly cycles.
const OPENING_SPAN: f64 = 30.0;
/// Where the timeline starts, as days from the first dose logged. A little before it, so the
/// first injection has the curve rising out of it rather than standing on the left edge.
const TIMELINE_OPENS: f64 = -2.0;
/// The shortest the value axis will run to, in micrograms. Ten times the level a curve is
/// followed down to, so a window past the end of a drug is ruled at figures that read as nothing.
const AXIS_FLOOR: f64 = TRACE * 10.0;

/// The doses the plan says are still to come, from the last one logged out to `horizon`.
///
/// Past the last rung the plan says nothing and the final strength is carried on as maintenance,
/// which is the one thing projected here that the user did not enter.
///
/// Only ever ahead of `now`: a dose that was due and never logged did not happen, and drawing it
/// would put a dose in the past that the log does not have.
fn projected(
    last: &Dose,
    origin: NaiveDate,
    interval: f64,
    now: f64,
    horizon: f64,
    plan: &[TitrationStep],
    started: Option<NaiveDate>,
) -> Vec<Given> {
    // Anchored on the last dose's own hour, so a plan keeps the time of day it is kept at.
    let from = instant_of(last, origin);
    let mut ahead = Vec::new();
    let mut cycle = 1;
    loop {
        let day = from + interval * f64::from(cycle);
        if day > horizon || ahead.len() >= 400 {
            return ahead;
        }
        if day >= now {
            // The plan decides the strength, so a step up shows before it is reached; past the
            // end of the plan the last strength holds. Read at the date the dose falls inside
            // rather than the midnight nearest it, so one given at midday takes that day's rung.
            let strength = chart::instant_at(day, origin)
                .map(|at| at.date())
                .zip(started)
                .and_then(|(date, started)| rung(plan, started, date))
                .map_or(last.micrograms, |rung| rung.micrograms);
            ahead.push(Given {
                day,
                micrograms: strength,
            });
        }
        cycle += 1;
    }
}

/// How much of the drug is still in the body, over the log behind and the plan ahead.
#[component]
#[allow(clippy::needless_pass_by_value)]
fn LevelCard(doses: Vec<Dose>, medication: Medication, clock: Clock) -> Element {
    // Unset until a gesture moves it. Fitted to the timeline below rather than on arrival, so the
    // one piece of code that decides what a window may be runs on every path into it.
    let mut window = use_signal(|| None::<(f64, f64)>);
    // Where the chart is being read, as the tap landed rather than as it is shown: how fine a
    // reading is worth quoting depends on the zoom, which the tap does not fix. Unset means now.
    let mut reading = use_signal(|| None::<f64>);

    use_hook(move || {
        spawn(async move {
            let mut gestures = document::eval(PLOT);
            while let Ok(moved) = gestures.recv::<Moved>().await {
                window.set(Some((moved.from, moved.to)));
                if let Some(day) = moved.read.filter(|day| day.is_finite()) {
                    reading.set(Some(day));
                }
            }
        });
    });

    // The oldest dose is day zero of the axis; everything else is measured from it.
    let Some(first) = doses.last() else {
        return rsx! {};
    };

    let origin = first.taken;
    let now = moment(origin);

    // The log is newest first, so the first match is the latest dose of that drug.
    let latest = |drug: Drug| doses.iter().find(|dose| dose.drug == Some(drug));

    // In the formulary's order rather than the order they were first taken, so an order taken
    // from the log cannot repaint one curve because a different one was logged.
    let taking: Vec<Drug> = Drug::ALL
        .into_iter()
        .filter(|drug| latest(*drug).is_some())
        .collect();
    // The drug the figures beside the chart describe: the one the medication record names, where
    // it has been logged. A record naming a drug never injected has nothing to project from and
    // settles nowhere, so it would describe a curve that is not on the chart.
    let Some(leading) = medication
        .drug
        .filter(|drug| latest(*drug).is_some())
        .or_else(|| taking.first().copied())
    else {
        return rsx! {};
    };

    // From the formulary rather than the medication record, since the record describes the drug
    // it names and that is not always the drug being drawn.
    let interval = f64::from(leading.interval_days().max(1));

    // The plan is the medication record's own drug's and reads across to no other.
    let prescribed = medication.drug == Some(leading);
    let plan: &[TitrationStep] = if prescribed {
        &medication.titration
    } else {
        &[]
    };
    let started = prescribed
        .then_some(medication.started)
        .flatten()
        .or(Some(first.taken));

    // The furthest anything is carried: a dozen cycles past the last injection, or past now where
    // the log has been left alone for longer than that.
    let cap = doses
        .first()
        .map_or(now, |dose| instant_of(dose, origin))
        .max(now)
        + interval * f64::from(PROJECTED_CYCLES);

    // Every drug's own doses, with the one under a plan carried forward on it.
    //
    // A dose having been given is no evidence that another will be, so without a plan every curve
    // washes out. Projected to the cap rather than to the horizon below, since the chart samples
    // a window either side of the one on show and the overscan would otherwise fall away for want
    // of doses the plan says are coming.
    let carried: Vec<(Drug, Kinetics, Vec<Given>)> = taking
        .iter()
        .map(|&drug| {
            let mut given = Given::from_log(&doses, origin, drug);
            if drug == leading && !plan.is_empty() {
                if let Some(last) = latest(drug) {
                    given.extend(projected(last, origin, interval, now, cap, plan, started));
                }
            }
            (drug, Kinetics::of(drug), given)
        })
        .collect();

    // The timeline runs out where the last of the drugs does, rather than at a fixed count of
    // cycles, since a curve not carried forward decays to nothing long before the cap. Every drug
    // is asked, not just the leading one, which may be outlived by the drug it replaced. Never
    // short of a cycle past now, so the dose next due always has somewhere to be drawn.
    let horizon = carried
        .iter()
        .map(|(_, kinetics, given)| {
            let last = given
                .iter()
                .map(|dose| dose.day)
                .fold(f64::NEG_INFINITY, f64::max);
            spent(*kinetics, given, last, cap)
        })
        .fold(now + interval, f64::max);

    // The timeline the window slides along. Refused past what a calendar holds, which is a dose
    // dated by a typo.
    let Some(closes) = chart::whole_day(horizon) else {
        return rsx! {};
    };
    let bounds = (TIMELINE_OPENS, closes);

    let (from, to) = chart::fit(
        window().unwrap_or((now - OPENING_SPAN / 2.0, now + OPENING_SPAN / 2.0)),
        bounds,
    );
    let span = to - from;

    // How fine the chart will speak at this zoom, and the reading rounded onto it. Snapped here
    // rather than where the tap arrives, so zooming in on a reading sharpens it.
    let grain = chart::grain(span);
    let cursor = reading()
        .map_or(now, |day| chart::snapped(day, grain))
        .clamp(bounds.0, bounds.1);

    let margin = span * f64::from(OVERSCAN);

    let drawn: Vec<(Drug, Kinetics, Vec<Given>, Vec<Sample>)> = carried
        .into_iter()
        .map(|(drug, kinetics, given)| {
            let samples = curve(
                kinetics,
                &given,
                from - margin,
                to + margin,
                RESOLUTION * (1 + 2 * OVERSCAN),
            );
            (drug, kinetics, given, samples)
        })
        .collect();

    // Across every curve, and to the window alone: fitting the axis to the overscan too would
    // scale the chart to a peak nobody can see, and change it as the window slid past one.
    let peak = drawn
        .iter()
        .flat_map(|(_, _, _, samples)| samples)
        .filter(|sample| sample.day >= from && sample.day <= to)
        .map(|sample| sample.micrograms)
        .fold(0.0_f64, f64::max);

    // A lone curve gets the band under it and a figure above it; two of either say less than one.
    let alone = drawn.len() == 1;

    // The leading drug's model and doses, taken from what was already built rather than rebuilt:
    // `Kinetics::of` solves for an absorption rate by halving an interval a hundred times.
    let front = drawn.iter().find(|(drug, ..)| *drug == leading);

    // Where the current strength settles, read at the two ends of a cycle: the trough just before
    // the next dose, and the peak at the drug's own time to peak after one.
    let settles = latest(leading)
        .filter(|_| alone)
        .zip(front)
        .map(|(last, (_, kinetics, ..))| {
            (
                kinetics.at_steady_state(last.micrograms, interval, interval),
                kinetics.peak_at_steady_state(last.micrograms, interval),
            )
        });

    // A window past the end of the drug holds nothing, and an axis fitted to that alone would
    // rule itself in micrograms.
    let ceiling = settles
        .map_or(peak, |(_, top)| peak.max(top))
        .max(AXIS_FLOOR);
    let Some(scale) = chart::Scale::new(from, to, ceiling * 1.15, CHART_WIDTH, CHART_HEIGHT) else {
        return rsx! {};
    };

    // A reading inside the same grain as now is now: at a day's grain that is anywhere today, and
    // at an hour's it is the hour the clock is in.
    let is_now = reading().is_none() || (cursor - now).abs() <= grain / 2.0;

    // How far up towards where this strength settles the level has come. A closure, since only
    // one of the four ways the line below reads quotes it.
    let settled_share = || {
        let Some((last, (_, kinetics, given, _))) = latest(leading).zip(front) else {
            return "now".to_owned();
        };
        let phase = (now - instant_of(last, origin)).clamp(0.0, interval);
        let settled = kinetics.at_steady_state(last.micrograms, interval, phase);
        let share = if settled > 0.0 {
            (on_board(*kinetics, given, now) / settled * 100.0).clamp(0.0, 999.0)
        } else {
            0.0
        };
        format!(
            "now · {share:.0}% of where {} mg settles",
            format_mg(last.micrograms)
        )
    };

    // The time of day only where the zoom earns it: at a day's grain a reading stands for the
    // whole day, and an hour beside it would be a figure the user never picked.
    let stamp = chart::instant_at(cursor, origin).map(|at| {
        if grain < 1.0 {
            format!("{} at {}", short_date(at.date()), at.format(clock.face()))
        } else {
            short_date(at.date())
        }
    });
    let when = match (stamp, is_now, alone) {
        (None, ..) => "an unreadable date".to_owned(),
        (_, true, true) => settled_share(),
        (_, true, false) => "now".to_owned(),
        (Some(stamp), false, _) if cursor > now => format!("projected for {stamp}"),
        (Some(stamp), false, _) => stamp,
    };

    // Only doses that were really given: a tick says an injection happened, which is not
    // something a plan can claim. Last, so the samples can be taken rather than copied.
    let series: Vec<chart::Series> = drawn
        .into_iter()
        .map(|(drug, kinetics, given, samples)| chart::Series {
            drug,
            samples,
            marks: doses
                .iter()
                .filter(|dose| dose.drug == Some(drug))
                .map(|dose| instant_of(dose, origin))
                .filter(|day| *day >= from - margin && *day <= to + margin)
                .collect(),
            pick: on_board(kinetics, &given, cursor),
        })
        .collect();

    rsx! {
        div { class: "card tight",
            h2 { class: "card-title", "Medication in circulation" }
            div { class: "level-head",
                // One drug reads as a figure; several read as a list, which doubles as the legend
                // so that identity is never carried by colour alone.
                if let (true, Some(only)) = (alone, series.first()) {
                    span { class: "level-value",
                        "{format_level(only.pick)}"
                        span { class: "hero-unit", "mg" }
                    }
                } else {
                    div { class: "level-list",
                        for drawn in series.iter() {
                            div {
                                key: "{drawn.drug:?}",
                                class: "level-row",
                                style: "color: var({drawn.drug.tint()})",
                                span { class: "level-swatch" }
                                span { class: "level-of", "{drawn.drug.label()}" }
                                span { class: "level-at", "{format_level(drawn.pick)} mg" }
                            }
                        }
                    }
                }
                // At the far edge: the moment all the figures are read at, and beside any one of
                // them it would read as that one's.
                p { class: "level-when", "{when}" }
            }

            chart::LevelPlot {
                series,
                origin,
                scale,
                now,
                bounds,
                settles,
                cursor: Some(cursor),
                clock,
                // Returning to now drops the reading with it, or the readout would name a day
                // that is no longer on screen.
                onnudge: move |nudge: chart::Nudge| {
                    if nudge == chart::Nudge::Now {
                        reading.set(None);
                    }
                    window.set(Some(nudge.from((from, to), now)));
                },
            }

        }
    }
}

#[component]
fn DoseRow(dose: Dose, today: NaiveDate, clock: Clock) -> Element {
    let title = match dose.time {
        Some(time) => format!(
            "{} at {}",
            day_label(dose.taken, today),
            format_time(time, clock)
        ),
        None => day_label(dose.taken, today),
    };
    // A log can span more than one drug, so the row says which. A dose with none is in no curve,
    // and this is where that is noticed.
    let sub = match dose.drug {
        Some(drug) => format!("{} · {}", drug.label(), dose.site.label()),
        None => format!("No drug set · {}", dose.site.label()),
    };
    rsx! {
        Row {
            icon: rsx! { Syringe { size: 20 } },
            title: "{title}",
            sub: "{sub}",
            meta: "{format_mg(dose.micrograms)} mg",
            target: SubPage::DoseDetail(dose.id),
        }
    }
}

#[component]
pub fn DosesPage() -> Element {
    let mut nav = use_context::<Nav>();
    let store = use_context::<Store>();
    let log = store.all();
    let today = today();
    let medication = store.medication();
    let clock = store.clock();
    let interval = medication.interval_days();

    // The plan is dated from the day the user gave it, or from the first dose if they gave none.
    let started = medication
        .started
        .or_else(|| log.last().map(|dose| dose.taken));
    let step = started.and_then(|started| rung(&medication.titration, started, today));
    let schedule = match (medication.drug, step) {
        (Some(drug), Some(step)) => format!(
            "{} · {} mg, week {} of {}",
            drug.label(),
            format_mg(step.micrograms),
            step.week,
            step.weeks
        ),
        (Some(drug), None) if medication.titration.is_empty() => {
            format!("{}, no plan entered", drug.label())
        }
        (Some(drug), None) => format!("{} · past the end of the plan", drug.label()),
        (None, _) => "Not set up yet".to_owned(),
    };

    let (sealed, open) = store.supply();
    let supply = match (sealed, open) {
        (0, 0) => "Nothing on the shelf".to_owned(),
        (sealed, 0) => format!("{sealed} sealed"),
        (0, open) => format!("{open} in use"),
        (sealed, open) => format!("{sealed} sealed · {open} in use"),
    };

    rsx! {
        if let Some(last) = log.first() {
            CycleCard { last: last.clone(), today, interval }
        } else {
            div { class: "card",
                h2 { class: "card-title", "Nothing logged yet" }
                p { class: "note",
                    "Log an injection and Atlas counts the cycle from it, here and on every page after."
                }
            }
        }

        // On the doses the chart can use, not on the medication record: a record naming a drug
        // that no dose carries leaves nothing to draw, and gating on it would hide this row too.
        if !log.is_empty() {
            if log.iter().any(|dose| dose.drug.is_some()) {
                LevelCard { doses: log.clone(), medication: medication.clone(), clock }
            } else {
                div { class: "card flush",
                    Row {
                        icon: rsx! { Pill { size: 20 } },
                        title: "Medication level",
                        sub: if medication.drug.is_some() {
                            "Needs the drug on your doses"
                        } else {
                            "Needs to know which drug you take"
                        },
                        target: SubPage::Medication,
                    }
                }
            }
        }

        button {
            class: "btn primary block",
            onclick: move |_| nav.push(SubPage::LogDose),
            if log.is_empty() { "Log your first dose" } else { "Log a dose" }
        }

        if store.status() == Status::Unreadable {
            p { class: "note",
                "This device is not letting Atlas store anything, so what you log here will last only until the app closes."
            }
        }

        if !log.is_empty() {
            div { class: "card tight",
                h2 { class: "card-title", "History" }
                div { class: "card flush",
                    for dose in log.iter() {
                        DoseRow { key: "{dose.id:?}", dose: dose.clone(), today, clock }
                    }
                }
            }
        }

        div { class: "card flush",
            Row {
                icon: rsx! { VialIcon { size: 20 } },
                title: "Inventory",
                sub: "{supply}",
                target: SubPage::Inventory,
            }
            Row {
                icon: rsx! { CalendarClock { size: 20 } },
                title: "Titration schedule",
                sub: "{schedule}",
                target: SubPage::Medication,
            }
        }
    }
}

#[component]
pub fn DoseDetailPage(id: DoseId) -> Element {
    let mut nav = use_context::<Nav>();
    let mut store = use_context::<Store>();
    // Deleting is two taps rather than a dialog: a dialog in the WebView would freeze the page
    // it is drawn over, and this record cannot be recovered.
    let mut arming = use_signal(|| false);

    let Some(dose) = store.get(id) else {
        return rsx! { MissingDose {} };
    };
    let previous = store.previous(id);
    let number = store.number(id).unwrap_or(1);
    let gap = previous.as_ref().map_or_else(
        || "—".to_owned(),
        |earlier| format!("{} d", (dose.taken - earlier.taken).num_days()),
    );
    // Only against the same drug: 2.5 mg of one to 5 mg of another is not a step up in dose.
    let change = previous
        .as_ref()
        .filter(|earlier| earlier.drug == dose.drug)
        .map_or_else(
            || "—".to_owned(),
            |earlier| strength_change(earlier.micrograms, dose.micrograms),
        );
    let when = match dose.time {
        Some(time) => format!(
            "{} at {}",
            long_date(dose.taken),
            format_time(time, store.clock())
        ),
        None => format!("{}, no time recorded", long_date(dose.taken)),
    };
    let what = match dose.drug {
        Some(drug) => drug.label().to_owned(),
        None => "No drug set".to_owned(),
    };
    let source = dose
        .from
        .and_then(|id| store.vial(id))
        .map(|(entry, vial)| {
            let volume = vial.draw(dose.micrograms).map_or_else(String::new, |ul| {
                format!(" · {} mL, {} units", format_volume(ul), format_units(ul))
            });
            format!("{}{volume}", entry.name())
        });

    rsx! {
        div { class: "card hero",
            div { class: "hero-main",
                span { class: "hero-value",
                    "{format_mg(dose.micrograms)}"
                    span { class: "hero-unit", "mg" }
                }
                span { class: "hero-label", "{what} · {when} · {dose.site.label()}" }
            }
        }

        div { class: "card tight",
            h2 { class: "card-title", "Against the log" }
            div { class: "stats",
                div { class: "stat",
                    span { class: "stat-value", "#{number}" }
                    span { class: "stat-label", "Dose" }
                }
                div { class: "stat",
                    span { class: "stat-value", "{gap}" }
                    span { class: "stat-label", "Since last" }
                }
                div { class: "stat",
                    span { class: "stat-value", "{change}" }
                    span { class: "stat-label", "Strength" }
                }
            }
        }

        if let Some(source) = source {
            div { class: "card tight",
                h2 { class: "card-title", "Drawn from" }
                p { class: "note", "{source}" }
            }
        }

        if !dose.note.is_empty() {
            div { class: "card",
                h2 { class: "card-title", "Notes" }
                p { class: "note", "{dose.note}" }
            }
        }

        button {
            class: "btn block",
            onclick: move |_| nav.push(SubPage::EditDose(id)),
            "Edit this dose"
        }
        button {
            class: if arming() { "btn block warn" } else { "btn block" },
            onclick: move |_| {
                if arming() {
                    store.remove(id);
                    nav.back();
                } else {
                    arming.set(true);
                }
            },
            if arming() { "Tap again to delete" } else { "Delete this dose" }
        }
    }
}

#[component]
pub fn LogDosePage() -> Element {
    rsx! {
        DoseForm {}
    }
}

#[component]
pub fn EditDosePage(id: DoseId) -> Element {
    let store = use_context::<Store>();
    if store.get(id).is_none() {
        return rsx! { MissingDose {} };
    }
    rsx! {
        DoseForm { id }
    }
}

/// Raw text rather than read figures, so a strength typed half way comes back half way.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Unsaved {
    pub date: String,
    pub time: String,
    pub strength: String,
    pub site: Site,
    pub drug: Option<Drug>,
    pub from: Option<VialId>,
    pub note: String,
}

/// The dose being entered, owned above the form because stepping out to mix or add a vial
/// unmounts it. Lasts while that form is still somewhere in the stack.
///
/// Reads peek rather than subscribe, or the form writing on every keystroke would set itself
/// re-rendering without end.
#[derive(Clone, Copy)]
pub struct Drafting {
    held: Signal<Option<(Option<DoseId>, Unsaved)>>,
}

impl Drafting {
    pub fn new() -> Self {
        Self {
            held: Signal::new(None),
        }
    }

    fn form(dose: Option<DoseId>) -> SubPage {
        dose.map_or(SubPage::LogDose, SubPage::EditDose)
    }

    fn hold(mut self, dose: Option<DoseId>, unsaved: Unsaved) {
        let next = Some((dose, unsaved));
        if *self.held.peek() != next {
            self.held.set(next);
        }
    }

    /// Driven from an effect on navigation, so a form abandoned and opened again starts clean.
    pub fn expire(mut self, nav: Nav) {
        // Read unconditionally: an effect run that skipped navigation is never woken again.
        let open = nav.open();
        let form = self.held.peek().as_ref().map(|(dose, _)| Self::form(*dose));
        if form.is_some_and(|form| !open.contains(&form)) {
            self.held.set(None);
        }
    }

    fn held(self, nav: Nav, dose: Option<DoseId>) -> Option<Unsaved> {
        if !nav.holds(Self::form(dose)) {
            return None;
        }
        self.held
            .peek()
            .as_ref()
            .filter(|(target, _)| *target == dose)
            .map(|(_, unsaved)| unsaved.clone())
    }
}

impl Default for Drafting {
    fn default() -> Self {
        Self::new()
    }
}

/// The form behind both logging and editing. `id` names the dose being rewritten; without one
/// the form adds a dose to the log. Nothing is saved until the button is pressed.
#[component]
fn DoseForm(id: Option<DoseId>) -> Element {
    let mut nav = use_context::<Nav>();
    let mut store = use_context::<Store>();
    let drafting = use_context::<Drafting>();

    let log = store.all();
    let existing = id.and_then(|id| store.get(id));

    // A new dose opens on now, since one is nearly always logged just after it was given.
    // Editing opens on what was recorded, and on no hour at all where none was.
    let default_date = existing.as_ref().map_or_else(today, |dose| dose.taken);
    let default_time = existing
        .as_ref()
        .map_or_else(|| Some(time_of_day()), |dose| dose.time);
    // The one field carried over from the last dose: a strength holds week to week and a date
    // does not.
    let default_strength = existing
        .as_ref()
        .or_else(|| log.first())
        .map(|dose| format_mg(dose.micrograms));
    let default_site = existing
        .as_ref()
        .map_or_else(|| suggest_site(&log), |dose| dose.site);
    // A new dose opens on the drug being taken; editing opens on what was recorded, including on
    // nothing.
    let default_drug = existing.as_ref().map_or_else(
        || {
            store
                .medication()
                .drug
                .or_else(|| log.first().and_then(|dose| dose.drug))
        },
        |dose| dose.drug,
    );
    let default_note = existing
        .as_ref()
        .map_or_else(String::new, |dose| dose.note.clone());

    let vials = store.open_vials();
    // Exactly one, or none: deducting from the wrong vial is a figure nobody asked for.
    let sole_match = || {
        let mut matching = vials
            .iter()
            .filter(|(entry, _)| entry.drug.is_some() && entry.drug == default_drug);
        matching
            .next()
            .filter(|_| matching.next().is_none())
            .map(|(_, vial)| vial.id)
    };
    let default_from = existing.as_ref().map_or_else(sole_match, |dose| dose.from);

    // Where the form was stepped out of to mix or add a vial, it opens on what it was left
    // holding rather than on the defaults above.
    let held = drafting.held(nav, id);
    let start_date = held.as_ref().map_or_else(
        || default_date.format("%Y-%m-%d").to_string(),
        |unsaved| unsaved.date.clone(),
    );
    // The clock setting stops here: the control both reports and expects 24-hour whatever face the
    // platform draws it with.
    let start_time = held.as_ref().map_or_else(
        || default_time.map(time_value).unwrap_or_default(),
        |unsaved| unsaved.time.clone(),
    );
    let start_strength = held.as_ref().map_or_else(
        || default_strength.unwrap_or_default(),
        |unsaved| unsaved.strength.clone(),
    );
    let start_site = held.as_ref().map_or(default_site, |unsaved| unsaved.site);
    let start_drug = held.as_ref().map_or(default_drug, |unsaved| unsaved.drug);
    let start_from = held.as_ref().map_or(default_from, |unsaved| unsaved.from);
    let start_note = held.map_or(default_note, |unsaved| unsaved.note);

    let mut date = use_signal(move || start_date);
    let mut time = use_signal(move || start_time);
    let mut strength = use_signal(move || start_strength);
    let mut site = use_signal(move || start_site);
    let mut drug = use_signal(move || start_drug);
    let mut note = use_signal(move || start_note);
    let mut from = use_signal(move || start_from);

    // Reads every box, so it runs again on every keystroke and the form is always held as it
    // stands. Stepping out to the inventory remounts this component; this outlives it.
    use_effect(move || {
        drafting.hold(
            id,
            Unsaved {
                date: date(),
                time: time(),
                strength: strength(),
                site: site(),
                drug: drug(),
                from: from(),
                note: note(),
            },
        );
    });

    let taken = NaiveDate::parse_from_str(&date(), "%Y-%m-%d").ok();
    let hour = parse_time(&time());
    let micrograms = parse_mg(&strength());
    let mistyped = micrograms.is_none() && !strength().trim().is_empty();
    let ready = taken.is_some() && micrograms.is_some();

    // Vials that could hold this dose, and whichever is picked whether it could or not: one
    // recorded before the drug was changed still has to be visible to be unpicked.
    let choices: Vec<(VialId, String)> = vials
        .iter()
        .filter(|(entry, vial)| fits(entry.drug, drug()) || from() == Some(vial.id))
        .map(|(entry, vial)| (vial.id, vial_label(store, entry, vial)))
        .collect();

    // Sealed vials this dose could have come from, where none is mixed yet. Mixing happens here
    // rather than on the Inventory page: leaving the form would remount it and lose what has
    // been entered so far.
    let mixable: Vec<Stock> = store
        .stock()
        .into_iter()
        .filter(|entry| entry.sealed > 0 && fits(entry.drug, drug()))
        .collect();

    let mismatch = from()
        .and_then(|id| store.vial(id))
        .and_then(|(entry, _)| entry.drug)
        .filter(|held| drug().is_some_and(|given| given != *held));

    let drawn =
        from()
            .and_then(|id| store.vial(id))
            .zip(micrograms)
            .map(|((_, vial), micrograms)| {
                let volume = vial.draw(micrograms).map(|microlitres| {
                    format!(
                        "Draw {} mL, {} units",
                        format_volume(microlitres),
                        format_units(microlitres)
                    )
                });
                // This dose's own draw is already counted against the vial; put it back before
                // asking whether the new strength fits.
                let returned = existing
                    .as_ref()
                    .filter(|dose| dose.from == Some(vial.id))
                    .map_or(0, |dose| dose.micrograms);
                let short = micrograms > store.remaining(&vial).saturating_add(returned);
                (volume, short)
            });

    rsx! {
        div { class: "card",
            div { class: "pair",
                div { class: "field",
                    label { r#for: "dose-date", "Date" }
                    input {
                        id: "dose-date",
                        r#type: "date",
                        value: "{date}",
                        oninput: move |event| date.set(event.value()),
                    }
                }
                div { class: "field",
                    label { r#for: "dose-time", "Time" }
                    input {
                        id: "dose-time",
                        r#type: "time",
                        value: "{time}",
                        oninput: move |event| time.set(event.value()),
                    }
                }
            }
            if hour.is_none() {
                p { class: "note",
                    "Without a time the level curve places this dose at midday."
                }
            }
            div { class: "field",
                label { r#for: "dose-strength", "Strength in mg" }
                input {
                    id: "dose-strength",
                    r#type: "text",
                    inputmode: "decimal",
                    placeholder: "2.5",
                    value: "{strength}",
                    oninput: move |event| strength.set(event.value()),
                }
                if mistyped {
                    p { class: "note", "A strength is a number of milligrams, such as 2.5." }
                }
            }
        }

        div { class: "card tight",
            h2 { class: "card-title", "Drug" }
            div { class: "chips",
                for option in Drug::ALL {
                    button {
                        class: if drug() == Some(option) { "chip on" } else { "chip" },
                        aria_pressed: "{drug() == Some(option)}",
                        // Tapping the chosen drug again unsets it: the only way back to a dose
                        // with no drug on it.
                        onclick: move |_| {
                            let given = (drug() != Some(option)).then_some(option);
                            drug.set(given);
                            let held = from().and_then(|id| store.vial(id)).and_then(|(entry, _)| entry.drug);
                            if held.is_some_and(|held| !fits(Some(held), given)) {
                                from.set(None);
                            }
                        },
                        "{option.label()}"
                    }
                }
            }
            if drug().is_none() {
                p { class: "note",
                    "Without a drug this dose stays in the log and out of the level curve: nothing can be modelled without knowing which molecule it was."
                }
            }
        }

        div { class: "card tight",
            h2 { class: "card-title", "Injection site" }
            div { class: "chips",
                for option in Site::ALL {
                    button {
                        class: if option == site() { "chip on" } else { "chip" },
                        aria_pressed: "{option == site()}",
                        onclick: move |_| site.set(option),
                        "{option.label()}"
                    }
                }
            }
            if !log.is_empty() {
                p { class: "note", "Atlas opens on the site you have gone longest without using." }
            }
        }

        div { class: "card tight",
            h2 { class: "card-title", "Drawn from" }
            if choices.is_empty() {
                if let Some(entry) = mixable.first().filter(|_| mixable.len() == 1) {
                    p { class: "note",
                        "{entry.sealed} sealed · {entry.name()}. Mix one here and this dose comes out of it; the log stays as you have it."
                    }
                    MixControl {
                        entry: entry.clone(),
                        // Straight onto the vial that was just made: mixing mid-dose is only
                        // ever done to draw from it.
                        onmixed: move |vial| from.set(Some(vial)),
                    }
                } else if !mixable.is_empty() {
                    p { class: "note",
                        "Nothing is mixed yet, and more than one group could be. Pick which on the Inventory page."
                    }
                    button {
                        class: "btn block",
                        onclick: move |_| nav.push(SubPage::Inventory),
                        "Open the inventory"
                    }
                } else {
                    p { class: "note",
                        "Nothing on the shelf this dose could have come from. Adding some keeps this form as you have it."
                    }
                    button {
                        class: "btn block",
                        onclick: move |_| nav.push(SubPage::AddStock),
                        "Add vials to inventory"
                    }
                }
            } else {
                div { class: "chips",
                    button {
                        class: if from().is_none() { "chip on" } else { "chip" },
                        aria_pressed: "{from().is_none()}",
                        onclick: move |_| from.set(None),
                        "Not from inventory"
                    }
                    for (id , name) in choices.iter().cloned() {
                        button {
                            key: "{id:?}",
                            class: if from() == Some(id) { "chip on" } else { "chip" },
                            aria_pressed: "{from() == Some(id)}",
                            onclick: move |_| from.set(Some(id)),
                            "{name}"
                        }
                    }
                }
                if let Some((volume, short)) = drawn {
                    if let Some(volume) = volume {
                        p { class: "note", "{volume}" }
                    }
                    if short {
                        p { class: "note caution",
                            "That is more than the vial has left. Atlas records the dose as you gave it and the vial as empty."
                        }
                    }
                    if let Some(held) = mismatch {
                        p { class: "note caution",
                            "This vial holds {held.label()}, and the dose above says something else. Atlas records both as you entered them."
                        }
                    }
                } else if from().is_none() {
                    p { class: "note",
                        "Saving takes nothing off the shelf. Pick a vial to draw this dose out of your stock."
                    }
                }
            }
        }

        div { class: "card",
            div { class: "field",
                label { r#for: "dose-notes", "Notes" }
                textarea {
                    id: "dose-notes",
                    rows: "3",
                    placeholder: "How did it go?",
                    value: "{note}",
                    oninput: move |event| note.set(event.value()),
                }
            }
        }

        button {
            class: "btn primary block",
            disabled: !ready,
            onclick: move |_| {
                let (Some(taken), Some(micrograms)) = (taken, micrograms) else {
                    return;
                };
                let draft = Draft {
                    taken,
                    time: hour,
                    drug: drug(),
                    micrograms,
                    site: site(),
                    from: from(),
                    note: note().trim().to_owned(),
                };
                match id {
                    Some(id) => store.update(id, draft),
                    None => {
                        store.add(draft);
                    }
                }
                nav.back();
            },
            if id.is_some() { "Save changes" } else { "Save dose" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::Page;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").expect("a date the test wrote")
    }

    #[test]
    fn the_last_two_days_are_named_and_everything_older_is_dated() {
        let today = date("2026-08-14");
        assert_eq!(day_label(today, today), "Today");
        assert_eq!(day_label(date("2026-08-13"), today), "Yesterday");
        assert_eq!(day_label(date("2026-08-06"), today), "Thursday 6 Aug");
    }

    #[test]
    fn a_date_ahead_of_today_is_dated_rather_than_named() {
        let today = date("2026-08-14");
        assert_eq!(day_label(date("2026-08-15"), today), "Saturday 15 Aug");
    }

    #[test]
    fn a_step_between_strengths_carries_its_direction() {
        assert_eq!(strength_change(1700, 2500), "+0.8 mg");
        assert_eq!(strength_change(2500, 1700), "−0.8 mg");
        assert_eq!(strength_change(2500, 2500), "Same");
    }

    #[test]
    fn a_vial_holds_a_dose_only_where_the_two_name_the_same_drug() {
        use Drug::{Semaglutide, Tirzepatide};

        assert!(fits(Some(Tirzepatide), Some(Tirzepatide)));
        assert!(
            !fits(Some(Semaglutide), Some(Tirzepatide)),
            "one drug was not drawn out of another"
        );
    }

    #[test]
    fn a_vial_or_a_dose_naming_no_drug_contradicts_nothing() {
        assert!(
            fits(None, Some(Drug::Tirzepatide)),
            "a vial Atlas cannot name"
        );
        assert!(
            fits(Some(Drug::Tirzepatide), None),
            "a dose with no drug set"
        );
        assert!(fits(None, None));
    }

    fn unsaved(strength: &str) -> Unsaved {
        Unsaved {
            date: "2026-08-16".to_owned(),
            time: "07:30".to_owned(),
            strength: strength.to_owned(),
            site: Site::LeftThigh,
            drug: Some(Drug::Tirzepatide),
            from: None,
            note: String::new(),
        }
    }

    fn drafting(form: SubPage, over: &[SubPage], f: impl FnOnce(Nav, Drafting)) {
        let dom = VirtualDom::new(|| rsx! { div {} });
        dom.in_runtime(|| {
            dioxus::core::Runtime::current().in_scope(ScopeId::ROOT, || {
                let mut nav = Nav::new();
                nav.push(form);
                for sub in over {
                    nav.push(*sub);
                }
                f(nav, Drafting::new());
            });
        });
    }

    #[test]
    fn a_form_that_was_never_left_holds_nothing() {
        drafting(SubPage::LogDose, &[], |nav, drafting| {
            assert_eq!(drafting.held(nav, None), None);
        });
    }

    #[test]
    fn what_was_typed_comes_back_as_it_was_typed() {
        drafting(SubPage::LogDose, &[], |nav, drafting| {
            drafting.hold(None, unsaved("2."));

            let back = drafting
                .held(nav, None)
                .expect("the form was left part-way");
            assert_eq!(back.strength, "2.", "a figure that is not yet a figure");
            assert_eq!(back, unsaved("2."));
        });
    }

    #[test]
    fn a_draft_survives_stepping_out_of_the_form_and_back() {
        drafting(
            SubPage::LogDose,
            &[SubPage::AddStock],
            |mut nav, drafting| {
                drafting.hold(None, unsaved("2.5"));
                assert!(
                    drafting.held(nav, None).is_some(),
                    "the form is still open under the one stepped into"
                );

                nav.back();
                assert_eq!(
                    drafting.held(nav, None).map(|u| u.strength),
                    Some("2.5".to_owned())
                );
            },
        );
    }

    #[test]
    fn leaving_the_form_discards_it_without_anyone_saying_so() {
        drafting(SubPage::LogDose, &[], |mut nav, drafting| {
            drafting.hold(None, unsaved("2.5"));

            nav.back();
            drafting.expire(nav);
            assert_eq!(drafting.held(nav, None), None, "the form is gone");

            nav.push(SubPage::LogDose);
            drafting.expire(nav);
            assert_eq!(
                drafting.held(nav, None),
                None,
                "and opening it again starts clean"
            );
        });
    }

    #[test]
    fn stepping_out_of_the_form_does_not_expire_the_draft() {
        drafting(SubPage::LogDose, &[], |mut nav, drafting| {
            drafting.hold(None, unsaved("2.5"));

            nav.push(SubPage::AddStock);
            drafting.expire(nav);
            nav.back();
            drafting.expire(nav);

            assert_eq!(
                drafting.held(nav, None).map(|u| u.strength),
                Some("2.5".to_owned())
            );
        });
    }

    #[test]
    fn a_draft_outlives_a_look_at_another_page() {
        drafting(SubPage::LogDose, &[], |mut nav, drafting| {
            drafting.hold(None, unsaved("2.5"));

            nav.go(Page::Profile);
            drafting.expire(nav);
            nav.go(Page::Doses);
            drafting.expire(nav);

            assert_eq!(
                drafting.held(nav, None).map(|u| u.strength),
                Some("2.5".to_owned()),
                "the form was still open the whole time"
            );
        });
    }

    #[test]
    fn a_draft_is_never_restored_onto_a_different_dose() {
        let three = DoseId::new(3);
        drafting(SubPage::EditDose(three), &[], |nav, drafting| {
            drafting.hold(Some(three), unsaved("2.5"));

            assert_eq!(
                drafting.held(nav, Some(three)).map(|u| u.strength),
                Some("2.5".to_owned())
            );
            assert_eq!(
                drafting.held(nav, Some(DoseId::new(4))),
                None,
                "another dose"
            );
            assert_eq!(drafting.held(nav, None), None, "nor a new one");
        });
    }
}
