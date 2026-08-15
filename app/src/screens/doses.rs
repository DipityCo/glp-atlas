//! The Doses page and the two sub-pages reached from it.

use crate::icons::{CalendarClock, Syringe};
use dioxus::prelude::*;

use super::Row;
use crate::nav::{Nav, SubPage};

/// A logged injection.
struct Dose {
    weekday: &'static str,
    date: &'static str,
    strength: &'static str,
    site: &'static str,
    note: &'static str,
}

static RECENT: [Dose; 4] = [
    Dose {
        weekday: "Thursday",
        date: "6 Aug",
        strength: "2.5 mg",
        site: "Left abdomen",
        note: "Mild nausea the following morning, settled by noon.",
    },
    Dose {
        weekday: "Thursday",
        date: "30 Jul",
        strength: "2.5 mg",
        site: "Right thigh",
        note: "No side effects.",
    },
    Dose {
        weekday: "Thursday",
        date: "23 Jul",
        strength: "1.7 mg",
        site: "Right abdomen",
        note: "Last week at the previous strength.",
    },
    Dose {
        weekday: "Friday",
        date: "17 Jul",
        strength: "1.7 mg",
        site: "Left thigh",
        note: "A day late, travelling.",
    },
];

/// Injection sites offered when logging a dose.
static SITES: [&str; 4] = ["Left abdomen", "Right abdomen", "Left thigh", "Right thigh"];

/// Days elapsed in the current seven-day cycle. Fills the countdown ring.
const DAYS_ELAPSED: u32 = 5;
const _: () = assert!(
    DAYS_ELAPSED <= 7,
    "the ring renders `7 - DAYS_ELAPSED`, which underflows past the end of the cycle"
);

#[component]
pub fn DosesPage() -> Element {
    let mut nav = use_context::<Nav>();
    let fill = DAYS_ELAPSED * 100 / 7;
    rsx! {
        div { class: "card hero",
            div { class: "ring", style: "--p:{fill}",
                div { class: "ring-inner",
                    span { class: "ring-num", "{7 - DAYS_ELAPSED}" }
                    span { class: "ring-cap", "days" }
                }
            }
            div { class: "hero-main",
                span { class: "hero-value",
                    "2.5"
                    span { class: "hero-unit", "mg" }
                }
                span { class: "hero-label", "Next dose Thursday, 13 Aug" }
            }
        }

        button {
            class: "btn primary block",
            onclick: move |_| nav.push(SubPage::LogDose),
            "Log this week's dose"
        }

        div { class: "card tight",
            h2 { class: "card-title", "Recent" }
            div { class: "card flush",
                for (index , dose) in RECENT.iter().enumerate() {
                    Row {
                        icon: rsx! { Syringe { size: 20 } },
                        title: "{dose.weekday} {dose.date}",
                        sub: "{dose.site}",
                        meta: "{dose.strength}",
                        target: SubPage::DoseDetail(index),
                    }
                }
            }
        }

        div { class: "card flush",
            Row {
                icon: rsx! { CalendarClock { size: 20 } },
                title: "Titration schedule",
                sub: "2.5 mg for 3 more weeks",
                target: SubPage::Medication,
            }
        }
    }
}

#[component]
pub fn DoseDetailPage(index: usize) -> Element {
    let Some(dose) = RECENT.get(index) else {
        return rsx! {
            div { class: "card",
                h2 { class: "card-title", "Dose not found" }
                p { class: "note", "This dose is no longer in the log." }
            }
        };
    };
    rsx! {
        div { class: "card hero",
            div { class: "hero-main",
                span { class: "hero-value",
                    "{dose.strength}"
                }
                span { class: "hero-label", "{dose.weekday} {dose.date} · {dose.site}" }
            }
        }
        div { class: "card",
            h2 { class: "card-title", "Notes" }
            p { class: "note", "{dose.note}" }
        }
        div { class: "card",
            h2 { class: "card-title", "Around this dose" }
            div { class: "stats",
                div { class: "stat",
                    span { class: "stat-value", "213.8" }
                    span { class: "stat-label", "Weight" }
                }
                div { class: "stat",
                    span { class: "stat-value", "7 d" }
                    span { class: "stat-label", "Since last" }
                }
                div { class: "stat",
                    span { class: "stat-value", "Mild" }
                    span { class: "stat-label", "Side effects" }
                }
            }
        }
        button { class: "btn block", "Edit this dose" }
    }
}

#[component]
pub fn LogDosePage() -> Element {
    let mut nav = use_context::<Nav>();
    let mut chosen = use_signal(|| 1usize);
    rsx! {
        div { class: "card",
            div { class: "field",
                label { r#for: "dose-date", "Date" }
                input { id: "dose-date", r#type: "date", value: "2026-08-13" }
            }
            div { class: "field",
                label { r#for: "dose-strength", "Strength" }
                input { id: "dose-strength", r#type: "text", value: "2.5 mg" }
            }
        }
        div { class: "card tight",
            h2 { class: "card-title", "Injection site" }
            div { class: "chips",
                for (index , site) in SITES.iter().enumerate() {
                    button {
                        class: if index == chosen() { "chip on" } else { "chip" },
                        aria_pressed: "{index == chosen()}",
                        onclick: move |_| chosen.set(index),
                        "{site}"
                    }
                }
            }
            p { class: "note", "Atlas suggests rotating away from the last two sites." }
        }
        div { class: "card",
            div { class: "field",
                label { r#for: "dose-notes", "Notes" }
                textarea { id: "dose-notes", rows: "3", placeholder: "How did it go?" }
            }
        }
        button {
            class: "btn primary block",
            onclick: move |_| nav.back(),
            "Save dose"
        }
    }
}
