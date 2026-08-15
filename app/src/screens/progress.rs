//! The Progress page and the two sub-pages reached from it.

use crate::icons::Ruler;
use dioxus::prelude::*;

use super::Row;
use crate::nav::{Nav, SubPage};

/// A weekly weigh-in.
struct WeighIn {
    label: &'static str,
    pounds: f32,
}

/// Weekly weigh-ins, oldest first.
#[rustfmt::skip]
static WEEKS: [WeighIn; 8] = [
    WeighIn { label: "Jun 18", pounds: 232.4 },
    WeighIn { label: "Jun 25", pounds: 230.1 },
    WeighIn { label: "Jul 2",  pounds: 228.6 },
    WeighIn { label: "Jul 9",  pounds: 226.0 },
    WeighIn { label: "Jul 16", pounds: 223.2 },
    WeighIn { label: "Jul 23", pounds: 221.5 },
    WeighIn { label: "Jul 30", pounds: 218.0 },
    WeighIn { label: "Aug 6",  pounds: 214.6 },
];

const START_WEIGHT: f32 = 233.0;
const GOAL_WEIGHT: f32 = 195.0;

/// A body measurement and its movement since the starting reading.
struct Measurement {
    name: &'static str,
    value: &'static str,
    change: &'static str,
}

#[rustfmt::skip]
static MEASUREMENTS: [Measurement; 4] = [
    Measurement { name: "Waist", value: "38.5 in", change: "−4.0 in" },
    Measurement { name: "Chest", value: "44.0 in", change: "−2.5 in" },
    Measurement { name: "Hips",  value: "42.0 in", change: "−3.0 in" },
    Measurement { name: "Neck",  value: "16.5 in", change: "−1.0 in" },
];

/// The weight range the chart is drawn over: a floor under the lowest reading so the shortest
/// bar still reads as a bar, and headroom above the highest.
fn chart_range() -> (f32, f32) {
    let low = WEEKS.iter().map(|w| w.pounds).fold(f32::MAX, f32::min);
    let high = WEEKS.iter().map(|w| w.pounds).fold(f32::MIN, f32::max);
    (low - 6.0, high + 1.0)
}

/// Scales a weigh-in to its share of the chart's vertical span.
fn bar_height(pounds: f32, low: f32, high: f32) -> f32 {
    ((pounds - low) / (high - low)) * 100.0
}

#[component]
pub fn ProgressPage() -> Element {
    let mut nav = use_context::<Nav>();
    let latest = WEEKS.len() - 1;
    let current = WEEKS[latest].pounds;
    let lost = START_WEIGHT - current;
    let remaining = current - GOAL_WEIGHT;
    let (low, high) = chart_range();
    rsx! {
        div { class: "card hero",
            div { class: "hero-main",
                span { class: "hero-value",
                    "{current:.1}"
                    span { class: "hero-unit", "lb" }
                }
                span { class: "hero-label", "Down {lost:.1} lb since 18 June" }
            }
        }

        div { class: "card tight",
            h2 { class: "card-title", "Last eight weeks" }
            div { class: "chart",
                for (index , height) in WEEKS.iter().map(|w| bar_height(w.pounds, low, high)).enumerate() {
                    div {
                        class: if index == latest { "bar last" } else { "bar" },
                        style: "height:{height:.1}%",
                    }
                }
            }
            div { class: "axis",
                for week in WEEKS.iter() {
                    span { "{week.label}" }
                }
            }
        }

        div { class: "stats",
            div { class: "stat",
                span { class: "stat-value", "{START_WEIGHT:.0}" }
                span { class: "stat-label", "Start" }
            }
            div { class: "stat",
                span { class: "stat-value", "{remaining:.1}" }
                span { class: "stat-label", "To goal" }
            }
            div { class: "stat",
                span { class: "stat-value", "2.3" }
                span { class: "stat-label", "lb / week" }
            }
        }

        button {
            class: "btn primary block",
            onclick: move |_| nav.push(SubPage::LogWeight),
            "Log today's weight"
        }

        div { class: "card flush",
            Row {
                icon: rsx! { Ruler { size: 20 } },
                title: "Measurements",
                sub: "Waist, chest, hips",
                meta: "3 wk ago",
                target: SubPage::Measurements,
            }
        }
    }
}

#[component]
pub fn LogWeightPage() -> Element {
    let mut nav = use_context::<Nav>();
    rsx! {
        div { class: "card",
            div { class: "field",
                label { r#for: "weight-value", "Weight" }
                input { id: "weight-value", r#type: "number", inputmode: "decimal", value: "214.6" }
            }
            div { class: "field",
                label { r#for: "weight-date", "Date" }
                input { id: "weight-date", r#type: "date", value: "2026-08-12" }
            }
        }
        div { class: "card tight",
            h2 { class: "card-title", "Compared with last week" }
            div { class: "stats",
                div { class: "stat",
                    span { class: "stat-value", "−3.4" }
                    span { class: "stat-label", "lb" }
                }
                div { class: "stat",
                    span { class: "stat-value", "−1.5%" }
                    span { class: "stat-label", "Change" }
                }
            }
        }
        button {
            class: "btn primary block",
            onclick: move |_| nav.back(),
            "Save weight"
        }
    }
}

#[component]
pub fn MeasurementsPage() -> Element {
    rsx! {
        div { class: "card flush",
            for entry in MEASUREMENTS.iter() {
                div { class: "row",
                    div { class: "row-main",
                        span { class: "row-title", "{entry.name}" }
                        span { class: "row-sub", "{entry.change} since June" }
                    }
                    span { class: "row-meta", "{entry.value}" }
                }
            }
        }
        p { class: "note", "Measured on the same morning as the weekly weigh-in." }
        button { class: "btn block", "Add a measurement" }
    }
}
