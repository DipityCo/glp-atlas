//! The app's icon set.
//!
//! Each icon is an outline with a small constellation over it. Two or three nodes carry an
//! icon; a fourth only where the vertices are themselves the subject, as in the trend line.
//! Magnitudes vary within every icon, and nodes sit where they reinforce the form rather than
//! on every vertex. All are stroked in `currentColor` on a 24×24 canvas and carry no colour of
//! their own.

use dioxus::prelude::*;

/// Outline weight. Anything under about 1.5 falls below a device pixel at the 15–20px sizes
/// these render at, and washes out.
const OUTLINE: &str = "1.5";
/// Outline opacity. The lines carry the shape; the stars mark it.
const FAINT: &str = "0.7";

/// Star magnitudes, brightest to faintest. The faintest is the floor at which a node still
/// covers a device pixel at the smallest size in use.
const M1: &str = "1.7";
const M2: &str = "1.4";
const M3: &str = "1.15";
const M4: &str = "0.95";

/// Shared shell. `outline` traces the shape, `stars` sits over it.
///
/// `size` is the design size in pixels at the default text size, emitted in `rem` so icons
/// follow the system font scale the way their containers do. Not `em`: `.tab` sets its own
/// font-size for the label beneath the icon, which would shrink a tab icon against every other.
#[component]
fn Constellation(size: u32, outline: Element, stars: Element) -> Element {
    let rem = f64::from(size) / 16.0;
    rsx! {
        svg {
            width: "{rem}rem",
            height: "{rem}rem",
            view_box: "0 0 24 24",
            fill: "none",
            // Spelled out: `dioxus_elements::svg` carries no `aria_hidden` shorthand.
            "aria-hidden": "true",
            g {
                stroke: "currentColor",
                stroke_width: OUTLINE,
                stroke_linecap: "round",
                stroke_linejoin: "round",
                opacity: FAINT,
                {outline}
            }
            g { fill: "currentColor", {stars} }
        }
    }
}

/// The barrel's far corners and the thumb rest. The needle carries no node, so its point stays
/// a point.
#[component]
pub fn Syringe(size: u32) -> Element {
    rsx! {
        Constellation {
            size,
            outline: rsx! {
                g { transform: "rotate(-45 12 12)",
                    path { d: "M3 12H8" }
                    rect { x: "8", y: "9.25", width: "8.5", height: "5.5", rx: "1.5" }
                    path { d: "M13 9.25v5.5" }
                    path { d: "M16.5 12h4" }
                    path { d: "M20.5 9.75v4.5" }
                }
            },
            stars: rsx! {
                g { transform: "rotate(-45 12 12)",
                    circle { cx: "8", cy: "14.5", r: M1 }
                    circle { cx: "16.5", cy: "9.25", r: M3 }
                    circle { cx: "20.5", cy: "14.25", r: M2 }
                }
            },
        }
    }
}

/// The one icon carrying four: on a trend line the vertices are the reading.
#[component]
pub fn ChartSpline(size: u32) -> Element {
    rsx! {
        Constellation {
            size,
            outline: rsx! {
                path { d: "M3.5 17L9 11.5l4.5 3L19.5 7" }
            },
            stars: rsx! {
                circle { cx: "19.5", cy: "7", r: M1 }
                circle { cx: "9", cy: "11.5", r: M2 }
                circle { cx: "3.5", cy: "17", r: M3 }
                circle { cx: "13.5", cy: "14.5", r: M4 }
            },
        }
    }
}

#[component]
pub fn UserRound(size: u32) -> Element {
    rsx! {
        Constellation {
            size,
            outline: rsx! {
                circle { cx: "12", cy: "8.25", r: "3.25" }
                path { d: "M5 19.75c0-3.87 3.13-7 7-7s7 3.13 7 7" }
            },
            stars: rsx! {
                circle { cx: "12", cy: "5", r: M1 }
                circle { cx: "19", cy: "19.75", r: M2 }
                circle { cx: "6.6", cy: "15.3", r: M4 }
            },
        }
    }
}

/// Both ends of the capsule, which is all the shape is.
#[component]
pub fn Pill(size: u32) -> Element {
    rsx! {
        Constellation {
            size,
            outline: rsx! {
                g { transform: "rotate(-45 12 12)",
                    rect { x: "2.5", y: "8", width: "19", height: "8", rx: "4" }
                    path { d: "M12 8v8" }
                }
            },
            stars: rsx! {
                g { transform: "rotate(-45 12 12)",
                    circle { cx: "20.5", cy: "14.5", r: M1 }
                    circle { cx: "3.6", cy: "9.4", r: M3 }
                }
            },
        }
    }
}

#[component]
pub fn Vial(size: u32) -> Element {
    rsx! {
        Constellation {
            size,
            outline: rsx! {
                path { d: "M9 2.5h6" }
                path { d: "M9.75 2.5v3.6" }
                path { d: "M14.25 2.5v3.6" }
                path {
                    d: "M9.75 6.1 8.2 8.4a3 3 0 0 0-.7 1.9V19a2.5 2.5 0 0 0 2.5 2.5h4a2.5 2.5 0 0 0 2.5-2.5v-8.7a3 3 0 0 0-.7-1.9l-1.55-2.3",
                }
                path { d: "M7.5 13.5h9" }
            },
            stars: rsx! {
                circle { cx: "16.5", cy: "19", r: M1 }
                circle { cx: "8.2", cy: "8.4", r: M2 }
                circle { cx: "7.5", cy: "13.5", r: M4 }
            },
        }
    }
}

#[component]
pub fn Ruler(size: u32) -> Element {
    rsx! {
        Constellation {
            size,
            outline: rsx! {
                g { transform: "rotate(-45 12 12)",
                    rect { x: "1.5", y: "9", width: "21", height: "6", rx: "1.5" }
                    path { d: "M6 9v2.5M10 9v2.5M14 9v2.5M18 9v2.5" }
                }
            },
            stars: rsx! {
                g { transform: "rotate(-45 12 12)",
                    circle { cx: "1.9", cy: "12", r: M1 }
                    circle { cx: "14", cy: "9", r: M3 }
                    circle { cx: "22.1", cy: "12", r: M2 }
                }
            },
        }
    }
}

#[component]
pub fn Bell(size: u32) -> Element {
    rsx! {
        Constellation {
            size,
            outline: rsx! {
                path { d: "M18 9.5a6 6 0 0 0-12 0c0 5.5-2.25 6.5-2.25 6.5h16.5S18 15 18 9.5" }
                path { d: "M13.8 19.5a2 2 0 0 1-3.6 0" }
            },
            stars: rsx! {
                circle { cx: "12", cy: "3.5", r: M1 }
                circle { cx: "4.4", cy: "16", r: M2 }
                circle { cx: "12", cy: "19.6", r: M3 }
            },
        }
    }
}

#[component]
pub fn CalendarClock(size: u32) -> Element {
    rsx! {
        Constellation {
            size,
            outline: rsx! {
                rect { x: "3.5", y: "5", width: "12", height: "12", rx: "1.75" }
                path { d: "M3.5 9.5h12" }
                path { d: "M7 3v3.5M12 3v3.5" }
                circle { cx: "17", cy: "16.5", r: "4" }
                path { d: "M17 14.9v1.7l1.3.8" }
            },
            stars: rsx! {
                circle { cx: "17", cy: "16.5", r: M1 }
                circle { cx: "3.5", cy: "9.5", r: M2 }
                circle { cx: "15.5", cy: "5", r: M4 }
            },
        }
    }
}

/// Point and tail, so the two nodes read as direction rather than decoration.
#[component]
pub fn ArrowLeft(size: u32) -> Element {
    rsx! {
        Constellation {
            size,
            outline: rsx! {
                path { d: "M19.5 12H5" }
                path { d: "M11 6l-6 6 6 6" }
            },
            stars: rsx! {
                circle { cx: "5", cy: "12", r: M1 }
                circle { cx: "19.5", cy: "12", r: M3 }
            },
        }
    }
}

#[component]
pub fn ArrowRight(size: u32) -> Element {
    rsx! {
        Constellation {
            size,
            outline: rsx! {
                path { d: "M4.5 12H19" }
                path { d: "M13 6l6 6-6 6" }
            },
            stars: rsx! {
                circle { cx: "19", cy: "12", r: M1 }
                circle { cx: "4.5", cy: "12", r: M3 }
            },
        }
    }
}

#[component]
pub fn ChevronRight(size: u32) -> Element {
    rsx! {
        Constellation {
            size,
            outline: rsx! {
                path { d: "M9.5 5.5L16 12l-6.5 6.5" }
            },
            stars: rsx! {
                circle { cx: "16", cy: "12", r: M1 }
                circle { cx: "9.5", cy: "18.5", r: M3 }
            },
        }
    }
}

#[component]
pub fn Check(size: u32) -> Element {
    rsx! {
        Constellation {
            size,
            outline: rsx! {
                path { d: "M4.5 12.5l5 5 10-11" }
            },
            stars: rsx! {
                circle { cx: "19.5", cy: "6.5", r: M1 }
                circle { cx: "9.5", cy: "17.5", r: M2 }
            },
        }
    }
}

/// Two arms, with the brightest node where they cross rather than at any end.
#[component]
pub fn Cross(size: u32) -> Element {
    rsx! {
        Constellation {
            size,
            outline: rsx! {
                path { d: "M6.5 6.5l11 11" }
                path { d: "M17.5 6.5l-11 11" }
            },
            stars: rsx! {
                circle { cx: "12", cy: "12", r: M1 }
                circle { cx: "6.5", cy: "6.5", r: M3 }
                circle { cx: "17.5", cy: "17.5", r: M4 }
            },
        }
    }
}
