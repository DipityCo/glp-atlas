//! The page bodies and the chrome that frames them: title bar, scrolling content column,
//! and the persistent tab bar.
//!
//! The Doses pages read and write the dose log in [`crate::store`]. Progress and Profile are
//! still layout over fixed sample figures.

mod doses;
mod profile;
mod progress;
mod style;

use crate::icons::{ArrowLeft, ChartSpline, ChevronRight, Syringe, UserRound};
use dioxus::prelude::*;

use crate::nav::{Nav, Page, SubPage};

pub use style::STYLESHEET;

/// The tab bar's icon for a page.
fn tab_icon(page: Page) -> Element {
    match page {
        Page::Doses => rsx! { Syringe { size: 23 } },
        Page::Progress => rsx! { ChartSpline { size: 23 } },
        Page::Profile => rsx! { UserRound { size: 23 } },
    }
}

/// Title bar for the current position, with a back control once a sub-page is open.
#[component]
pub fn TopBar() -> Element {
    let mut nav = use_context::<Nav>();
    let (title, subtitle) = match nav.top() {
        Some(sub) => (sub.title(), sub.subtitle()),
        None => (nav.page().title(), nav.page().subtitle()),
    };
    rsx! {
        header { class: "topbar",
            if nav.depth() > 0 {
                button {
                    class: "back",
                    aria_label: "Back",
                    onclick: move |_| nav.back(),
                    ArrowLeft { size: 21 }
                }
            }
            div {
                h1 { "{title}" }
                p { class: "subtitle", "{subtitle}" }
            }
        }
    }
}

/// Tab bar over the three top-level destinations, in the order they appear across the field.
#[component]
pub fn TabBar() -> Element {
    let mut nav = use_context::<Nav>();
    let current = nav.page();
    rsx! {
        nav { class: "tabbar",
            for page in Page::ALL {
                button {
                    class: if page == current { "tab active" } else { "tab" },
                    aria_current: if page == current { "page" },
                    onclick: move |_| nav.go(page),
                    {tab_icon(page)}
                    span { "{page.title()}" }
                }
            }
        }
    }
}

/// A list row that pushes a sub-page. `sub` and `meta` default to absent.
#[component]
pub fn Row(
    icon: Element,
    title: String,
    sub: Option<String>,
    meta: Option<String>,
    target: SubPage,
) -> Element {
    let mut nav = use_context::<Nav>();
    rsx! {
        button { class: "row", onclick: move |_| nav.push(target),
            div { class: "marker", {icon} }
            div { class: "row-main",
                span { class: "row-title", "{title}" }
                if let Some(sub) = sub {
                    span { class: "row-sub", "{sub}" }
                }
            }
            if let Some(meta) = meta {
                span { class: "row-meta", "{meta}" }
            }
            span { class: "chevron", ChevronRight { size: 20 } }
        }
    }
}

/// The body for the current navigation position.
#[component]
pub fn Body() -> Element {
    let nav = use_context::<Nav>();
    match nav.top() {
        None => match nav.page() {
            Page::Doses => rsx! { doses::DosesPage {} },
            Page::Progress => rsx! { progress::ProgressPage {} },
            Page::Profile => rsx! { profile::ProfilePage {} },
        },
        Some(SubPage::DoseDetail(id)) => rsx! { doses::DoseDetailPage { id } },
        Some(SubPage::LogDose) => rsx! { doses::LogDosePage {} },
        Some(SubPage::EditDose(id)) => rsx! { doses::EditDosePage { id } },
        Some(SubPage::LogWeight) => rsx! { progress::LogWeightPage {} },
        Some(SubPage::Measurements) => rsx! { progress::MeasurementsPage {} },
        Some(SubPage::Medication) => rsx! { profile::MedicationPage {} },
        Some(SubPage::Reminders) => rsx! { profile::RemindersPage {} },
    }
}
