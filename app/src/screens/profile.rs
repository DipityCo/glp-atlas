//! The Profile page and the two sub-pages reached from it.

use crate::icons::{ArrowRight, Bell, Check, Pill};
use dioxus::prelude::*;

use super::Row;
use crate::nav::SubPage;

/// One rung of the titration ladder.
struct TitrationStep {
    strength: &'static str,
    window: &'static str,
    done: bool,
}

#[rustfmt::skip]
static TITRATION: [TitrationStep; 5] = [
    TitrationStep { strength: "0.25 mg", window: "Weeks 1–4",   done: true },
    TitrationStep { strength: "0.5 mg",  window: "Weeks 5–8",   done: true },
    TitrationStep { strength: "1.0 mg",  window: "Weeks 9–12",  done: true },
    TitrationStep { strength: "1.7 mg",  window: "Weeks 13–16", done: true },
    TitrationStep { strength: "2.5 mg",  window: "Weeks 17–20", done: false },
];

#[component]
pub fn ProfilePage() -> Element {
    let mut units_metric = use_signal(|| false);
    rsx! {
        div { class: "card flush",
            Row {
                icon: rsx! { Pill { size: 20 } },
                title: "Medication",
                sub: "Wegovy · 2.5 mg weekly",
                target: SubPage::Medication,
            }
            Row {
                icon: rsx! { Bell { size: 20 } },
                title: "Reminders",
                sub: "Thursdays at 8:00",
                target: SubPage::Reminders,
            }
        }

        div { class: "card tight",
            h2 { class: "card-title", "Units" }
            div { class: "chips",
                button {
                    class: if units_metric() { "chip" } else { "chip on" },
                    aria_pressed: "{!units_metric()}",
                    onclick: move |_| units_metric.set(false),
                    "Pounds"
                }
                button {
                    class: if units_metric() { "chip on" } else { "chip" },
                    aria_pressed: "{units_metric()}",
                    onclick: move |_| units_metric.set(true),
                    "Kilograms"
                }
            }
        }

        div { class: "card flush",
            div { class: "row",
                div { class: "row-main",
                    span { class: "row-title", "Export data" }
                    span { class: "row-sub", "CSV of doses and weigh-ins" }
                }
            }
            div { class: "row",
                div { class: "row-main",
                    span { class: "row-title", "About Atlas" }
                    span { class: "row-sub", "Version 0.1.0" }
                }
            }
        }

        p { class: "note",
            "Everything stays on this device. Atlas is a log, not medical advice; dose changes belong with your prescriber."
        }
    }
}

#[component]
pub fn MedicationPage() -> Element {
    rsx! {
        div { class: "card",
            div { class: "field",
                label { r#for: "med-name", "Medication" }
                input { id: "med-name", r#type: "text", value: "Wegovy (semaglutide)" }
            }
            div { class: "field",
                label { r#for: "med-dose-day", "Dose day" }
                input { id: "med-dose-day", r#type: "text", value: "Thursday" }
            }
        }
        div { class: "card tight",
            h2 { class: "card-title", "Titration" }
            div { class: "card flush",
                for step in TITRATION.iter() {
                    div { class: "row",
                        div { class: if step.done { "marker dim" } else { "marker" },
                            if step.done {
                                Check { size: 17 }
                            } else {
                                ArrowRight { size: 17 }
                            }
                        }
                        div { class: "row-main",
                            span { class: "row-title", "{step.strength}" }
                            span { class: "row-sub", "{step.window}" }
                        }
                    }
                }
            }
        }
        button { class: "btn block", "Edit schedule" }
    }
}

#[component]
pub fn RemindersPage() -> Element {
    let mut dose_reminder = use_signal(|| true);
    let mut weigh_in = use_signal(|| true);
    let mut refill = use_signal(|| false);
    rsx! {
        div { class: "card flush",
            div { class: "row",
                div { class: "row-main",
                    span { class: "row-title", "Dose day" }
                    span { class: "row-sub", "Thursdays at 8:00" }
                }
                button {
                    class: if dose_reminder() { "toggle on" } else { "toggle" },
                    aria_label: "Dose day reminder",
                    aria_pressed: "{dose_reminder()}",
                    onclick: move |_| dose_reminder.set(!dose_reminder()),
                }
            }
            div { class: "row",
                div { class: "row-main",
                    span { class: "row-title", "Weigh-in" }
                    span { class: "row-sub", "Wednesdays at 7:00" }
                }
                button {
                    class: if weigh_in() { "toggle on" } else { "toggle" },
                    aria_label: "Weigh-in reminder",
                    aria_pressed: "{weigh_in()}",
                    onclick: move |_| weigh_in.set(!weigh_in()),
                }
            }
            div { class: "row",
                div { class: "row-main",
                    span { class: "row-title", "Refill" }
                    span { class: "row-sub", "Five days before the pen runs out" }
                }
                button {
                    class: if refill() { "toggle on" } else { "toggle" },
                    aria_label: "Refill reminder",
                    aria_pressed: "{refill()}",
                    onclick: move |_| refill.set(!refill()),
                }
            }
        }
        p { class: "note", "Reminders are local notifications; nothing leaves the device." }
    }
}
