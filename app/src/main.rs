//! GLP Atlas: a GLP-1 dose and progress tracker.
//!
//! The app sits inside one star field: the three top-level pages lie side by side in it and
//! sub-pages sit deeper in, so navigating is a camera move. [`sky`] draws the field, [`nav`]
//! holds the position, [`screens`] draws everything in front, and [`store`] holds the dose log
//! that outlives the session.

mod chart;
mod formulary;
mod icons;
mod kinetics;
mod nav;
mod screens;
mod sky;
mod stock;
mod store;
mod units;

use dioxus::document;
use dioxus::prelude::*;

use nav::{Direction, Nav};
use screens::{Body, Drafting, TabBar, TopBar, STYLESHEET};
use sky::Sky;
use store::use_store;

/// Reports the direction of a committed swipe.
const GESTURE: &str = include_str!("gesture.js");

/// Installs the hook `MainActivity` calls when the platform back gesture fires.
const BACK_BRIDGE: &str = "window.__atlasBack = () => dioxus.send(1);";

fn main() {
    #[cfg(target_os = "android")]
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );
    dioxus::launch(App);
}

#[allow(non_snake_case)]
fn App() -> Element {
    let nav = use_context_provider(Nav::new);
    let drafting = use_context_provider(Drafting::new);
    use_effect(move || drafting.expire(nav));
    use_store();

    // `use_hook`, not `use_future`: a future re-runs when a signal its body read changes,
    // and both of these navigate, which would reinstall the listeners on every navigation.
    use_hook(move || {
        spawn(async move {
            let mut nav = nav;
            let mut requests = document::eval(BACK_BRIDGE);
            while requests.recv::<u32>().await.is_ok() {
                nav.back();
            }
        });

        spawn(async move {
            let mut nav = nav;
            let mut swipes = document::eval(GESTURE);
            while let Ok(crossed) = swipes.recv::<i32>().await {
                // `gesture.js` signs a committed swipe by the direction it crossed in:
                // positive lands later in page order.
                let direction = if crossed > 0 {
                    Direction::Next
                } else {
                    Direction::Previous
                };
                if let Some(page) = nav.neighbour(direction) {
                    nav.go(page);
                }
            }
        });
    });

    let camera = nav.camera();

    rsx! {
        // `viewport-fit=cover` so `env(safe-area-inset-*)` reports real insets.
        document::Meta {
            name: "viewport",
            content: "width=device-width, initial-scale=1.0, viewport-fit=cover",
        }
        style { {STYLESHEET} }
        div {
            // `dragging` is added and removed by the gesture alone.
            class: "app",
            // Read by gesture.js for the resting camera, and by MainActivity for data-back.
            style: "--pan:{camera.pan};--depth:{camera.depth}",
            "data-pages": "{nav::Page::ALL.len()}",
            "data-back": if nav.can_go_back() { "1" } else { "0" },

            Sky {}
            // The one-element loop is load-bearing: keys only take effect inside a list, so this
            // is what remounts the shell on each navigation and replays its enter animation.
            for step in [nav.step()] {
                div { key: "{step}", class: nav.motion().class(),
                    TopBar {}
                    main { class: "content",
                        div { class: "column", Body {} }
                    }
                }
            }
            TabBar {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::PLOT;

    // The scripts, the stylesheet and the chart cannot see each other, so these read one
    // another's source for the names they agree on.
    const CHART: &str = include_str!("chart.rs");

    #[test]
    fn the_gesture_script_and_the_stylesheet_agree() {
        for selector in [".app", ".shell", ".sky-plane", ".sky-haze", ".sky-glow"] {
            assert!(
                GESTURE.contains(selector),
                "gesture.js no longer reaches for `{selector}`"
            );
            assert!(
                STYLESHEET.contains(selector),
                "`{selector}` has no rule in the stylesheet"
            );
        }

        for property in ["--pan", "--depth", "--pan-step", "--parallax"] {
            assert!(
                GESTURE.contains(property),
                "gesture.js no longer reads `{property}`"
            );
            assert!(
                STYLESHEET.contains(&format!("{property}:")),
                "`{property}` is never set in the stylesheet"
            );
        }

        assert!(GESTURE.contains(r#"classList.add("dragging")"#));
        assert!(
            STYLESHEET.contains(".app.dragging"),
            "the class a drag adds has no rule"
        );
    }

    #[test]
    fn the_swipe_opt_out_matches_the_chart_that_claims_it() {
        assert!(
            GESTURE.contains("data-swipe='off'"),
            "gesture.js no longer honours the opt-out"
        );
        assert!(
            CHART.contains(r#""data-swipe": "off""#),
            "nothing opts out, so the attribute is dead"
        );
    }

    #[test]
    fn the_gesture_script_reads_only_attributes_the_chart_writes() {
        for (attribute, dataset) in [
            ("data-plot", "[data-plot]"),
            ("data-plot-layer", "[data-plot-layer]"),
            ("data-from", "\"from\""),
            ("data-to", "\"to\""),
            ("data-floor", "\"floor\""),
            ("data-ceiling", "\"ceiling\""),
            ("data-drawn-from", "\"drawnFrom\""),
            ("data-drawn-to", "\"drawnTo\""),
            ("data-min-span", "\"minSpan\""),
            ("data-max-span", "\"maxSpan\""),
            ("data-view-width", "\"viewWidth\""),
            ("data-pad-left", "\"padLeft\""),
            ("data-pad-right", "\"padRight\""),
        ] {
            assert!(
                CHART.contains(attribute),
                "the chart no longer writes `{attribute}`"
            );
            assert!(
                PLOT.contains(dataset),
                "plot.js no longer reads `{attribute}`"
            );
        }
    }

    #[test]
    fn the_markers_the_gesture_scripts_select_on_carry_a_value() {
        for marker in [
            "data-plot",
            "data-plot-layer",
            "data-plot-dates",
            "data-swipe",
        ] {
            assert!(
                !CHART.contains(&format!("\"{marker}\": \"\"")),
                "`{marker}` is set to nothing, so nothing will match it"
            );
            assert!(
                CHART.contains(&format!("\"{marker}\": \"")),
                "the chart no longer carries `{marker}`"
            );
        }
    }

    #[test]
    fn the_gesture_scripts_answer_both_a_finger_and_a_mouse() {
        for script in [GESTURE, PLOT] {
            for event in ["touchstart", "touchmove", "mousedown", "mousemove"] {
                assert!(
                    script.contains(event),
                    "a script deaf to `{event}` is dead on one of the two"
                );
            }
            assert!(
                script.contains("TOUCH_TAIL_MS"),
                "a touch fires mouse events after itself, and unguarded they are a second gesture"
            );
            assert!(
                !script.contains("let touching"),
                "a latch never released leaves the mouse dead for the session on a device with both"
            );
        }
    }

    #[test]
    fn the_gesture_scripts_place_a_touch_by_where_it_fell() {
        for script in [GESTURE, PLOT] {
            assert!(
                script.contains("getBoundingClientRect"),
                "a script that hit-tests instead will miss the empty parts of the chart"
            );
        }
        assert!(!PLOT.contains("event.target"));
    }

    #[test]
    fn the_gesture_scripts_each_clean_up_after_themselves() {
        for script in [GESTURE, PLOT] {
            assert!(script.contains("__atlas"));
            assert!(
                script.contains("Cleanup"),
                "a script with no teardown doubles its listeners on every install"
            );
        }
    }

    #[test]
    fn the_moving_layer_is_clipped_by_something_that_does_not_move() {
        let clip = CHART
            .find(r#""clip-path": "url(#atlas-plot-window)""#)
            .expect("nothing clips the chart, so the overscan is simply visible");
        let layer = CHART
            .find(r#""data-plot-layer""#)
            .expect("the chart no longer carries a moving layer");

        assert!(
            CHART.contains(r#"id: "atlas-plot-window""#),
            "the clip is named"
        );
        assert!(
            clip < layer,
            "the clip moves with the layer, so it cuts nothing"
        );
    }

    #[test]
    fn the_date_labels_are_somewhere_the_gesture_can_move_them() {
        assert!(
            CHART.contains(r#""data-plot-dates": "days""#),
            "the chart no longer marks the labels out for the script"
        );
        assert!(
            PLOT.contains("[data-plot-dates] text"),
            "plot.js no longer reaches for the labels"
        );
        assert!(
            PLOT.contains(r#"getAttribute("x")"#),
            "plot.js moves the labels from somewhere other than where the chart put them"
        );
    }

    #[test]
    fn the_activity_lets_the_page_store_the_log() {
        const ACTIVITY: &str = include_str!("../android/MainActivity.kt");

        assert!(
            ACTIVITY.contains("domStorageEnabled = true"),
            "the WebView will refuse the storage the log is kept in"
        );
    }

    #[test]
    fn the_back_bridge_matches_the_activity() {
        const ACTIVITY: &str = include_str!("../android/MainActivity.kt");

        assert!(BACK_BRIDGE.contains("__atlasBack"));
        assert!(
            ACTIVITY.contains("__atlasBack"),
            "the activity calls a hook the app never installs"
        );
        assert!(
            ACTIVITY.contains("dataset.back"),
            "the activity no longer reads the app's back state"
        );
    }
}
