//! GLP Atlas: a GLP-1 dose and progress tracker.
//!
//! The app sits inside one star field: the three top-level pages lie side by side in it and
//! sub-pages sit deeper in, so navigating is a camera move. [`sky`] draws the field, [`nav`]
//! holds the position, [`screens`] draws everything in front.

mod icons;
mod nav;
mod screens;
mod sky;

use dioxus::document;
use dioxus::prelude::*;

use nav::{Direction, Nav};
use screens::{Body, TabBar, TopBar, STYLESHEET};
use sky::Sky;

/// Swipe-to-pan gesture. Reports the direction of a committed swipe.
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

    // `use_hook`, not `use_future`: a future re-runs when a signal its body read changes,
    // and both of these navigate, which would reinstall the listeners on every navigation.
    use_hook(move || {
        // Called by MainActivity when the platform back gesture has somewhere to go.
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
            // The one-element loop is load-bearing: keys only take effect inside a list, so
            // this is what makes the shell remount on each navigation and replay its enter
            // animation. A bare `div` with a `key` would diff in place and never animate.
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

    /// `gesture.js` drives the field by hand, reaching for classes and custom properties the
    /// stylesheet owns. Neither file can see the other, so a rename on either side leaves
    /// swiping silently inert.
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

    /// The activity reaches into the page for a hook the app installs, by name.
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
