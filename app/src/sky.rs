//! The twilight star field the app sits on.
//!
//! A stack of parallax planes. Each plane carries the camera transform; the box of stars
//! inside it carries the drift, so ambient motion and navigation never interrupt each other.
//! Placement is generated once from a fixed seed, so the field is identical on every run.

use std::sync::OnceLock;

use dioxus::prelude::*;

/// How strongly a plane answers the camera: 0 is immobile, 1 is the front plane. The
/// stylesheet multiplies the camera's pan and depth by it.
fn parallax_style(parallax: f32) -> String {
    format!("--parallax:{parallax}")
}

/// A star, reduced to what the renderer needs.
struct Star {
    class: &'static str,
    style: String,
}

/// Ambient motion inside a plane: amplitude as a percentage of the box, rotation in
/// degrees, and the period of one sweep in seconds.
struct Drift {
    x: f32,
    y: f32,
    rotation: f32,
    period: f32,
}

/// One parallax plane.
struct PlaneSpec {
    seed: u64,
    count: usize,
    /// 0 = infinitely far (immobile), 1 = the front plane.
    parallax: f32,
    /// Size of the star box as a percentage of the viewport. Covers the plane's own pan and
    /// drift only. The camera never scales below 1, so zoom needs no margin.
    box_size: (f32, f32),
    /// Diameter range in pixels.
    size: (f32, f32),
    /// Peak-opacity range before the horizon wash is applied.
    opacity: (f32, f32),
    /// Fraction of stars that twinkle; the rest are drawn once. The field's main cost dial:
    /// a twinkling star runs a compositable animation and becomes a layer of its own.
    twinkle_share: f32,
    /// Twinkle period range in seconds.
    period: (f32, f32),
    /// `None` leaves the box static, painting into the plane's layer rather than its own.
    drift: Option<Drift>,
    /// Whether stars carry a `box-shadow` halo.
    glow: bool,
    /// Every Nth star gets a diffraction flare. Zero disables them.
    flare_every: usize,
}

static PLANES: [PlaneSpec; 3] = [
    PlaneSpec {
        seed: 0x5EED_0A71,
        count: 106,
        parallax: 0.16,
        box_size: (116.0, 112.0),
        size: (0.8, 1.8),
        opacity: (0.22, 0.62),
        // At under 2px a twinkle is invisible.
        twinkle_share: 0.0,
        period: (5.0, 11.0),
        // Imperceptible at this parallax, and costs a composited layer to run.
        drift: None,
        glow: false,
        flare_every: 0,
    },
    PlaneSpec {
        seed: 0x5EED_0B32,
        count: 62,
        parallax: 0.52,
        box_size: (136.0, 112.0),
        size: (1.4, 2.6),
        opacity: (0.34, 0.85),
        twinkle_share: 0.15,
        period: (4.0, 9.0),
        drift: Some(Drift {
            x: 1.2,
            y: 0.6,
            rotation: 0.35,
            period: 380.0,
        }),
        glow: true,
        flare_every: 0,
    },
    PlaneSpec {
        seed: 0x5EED_0C19,
        count: 32,
        parallax: 1.0,
        box_size: (164.0, 112.0),
        size: (2.1, 3.8),
        opacity: (0.50, 1.0),
        twinkle_share: 0.85,
        period: (3.5, 7.5),
        drift: Some(Drift {
            x: 2.0,
            y: 0.9,
            rotation: 0.22,
            period: 250.0,
        }),
        glow: true,
        flare_every: 6,
    },
];

/// The afterglow drifts wider and faster than any star plane, so the bright side of the sky
/// sits somewhere different on each page.
const GLOW_PARALLAX: f32 = 0.34;
static GLOW_DRIFT: Drift = Drift {
    x: 4.0,
    y: 1.2,
    rotation: 0.0,
    period: 320.0,
};

const HAZE_PARALLAX: f32 = 0.3;
static HAZE_DRIFT: Drift = Drift {
    x: 2.6,
    y: 1.0,
    rotation: 0.6,
    period: 460.0,
};

/// Star colours, weighted toward the cool end with a few warm outliers.
static TINTS: [&str; 6] = [
    "#f4f6ff", "#dfe6ff", "#ffffff", "#c9d6ff", "#ffe6cf", "#ffd9c2",
];

/// xorshift64*, seeded per plane so the field never changes between runs.
struct Rng(u64);

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }

    /// A value in `[0, 1)`, built from 24 bits so it is exactly representable.
    fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }

    fn index(&mut self, len: usize) -> usize {
        self.next_u32() as usize % len
    }
}

/// Lays out one plane's stars. Density and brightness fall off toward the bright horizon.
fn generate(spec: &PlaneSpec) -> Vec<Star> {
    let mut rng = Rng::new(spec.seed);
    (0..spec.count)
        .map(|i| {
            let x = rng.unit() * 100.0;
            let y = rng.unit().powf(1.15) * 100.0;
            let size = rng.range(spec.size.0, spec.size.1);
            let wash = 1.0 - 0.45 * (y / 100.0);
            let bright = rng.range(spec.opacity.0, spec.opacity.1) * wash;
            let dim = bright * rng.range(0.25, 0.6);
            let twinkles = rng.unit() < spec.twinkle_share;
            let period = rng.range(spec.period.0, spec.period.1);
            let offset = rng.range(0.0, 12.0);
            let tint = TINTS[rng.index(TINTS.len())];
            let flare = spec.flare_every > 0 && i % spec.flare_every == 0;

            let animation = if twinkles {
                format!(";animation-duration:{period:.2}s;animation-delay:-{offset:.2}s")
            } else {
                String::new()
            };
            let halo = if spec.glow {
                format!(";box-shadow:0 0 {:.1}px {tint}", size * 2.2)
            } else {
                String::new()
            };
            let style = format!(
                "left:{x:.3}%;top:{y:.3}%;width:{size:.2}px;height:{size:.2}px;\
                 --tint:{tint};--o-dim:{dim:.3};--o-bright:{bright:.3};opacity:{bright:.3}\
                 {halo}{animation}"
            );

            let class = match (twinkles, flare) {
                (true, true) => "star twinkle flare",
                (true, false) => "star twinkle",
                (false, true) => "star flare",
                (false, false) => "star",
            };
            Star { class, style }
        })
        .collect()
}

/// The star field, generated once for the life of the process.
fn field() -> &'static [Vec<Star>] {
    static FIELD: OnceLock<Vec<Vec<Star>>> = OnceLock::new();
    FIELD.get_or_init(|| PLANES.iter().map(generate).collect())
}

/// Drift animation parameters.
fn drift_style(drift: &Drift) -> String {
    let Drift {
        x,
        y,
        rotation,
        period,
    } = *drift;
    format!("--dx:{x:.2}%;--dy:{y:.2}%;--rot:{rotation:.2}deg;animation-duration:{period:.0}s")
}

/// The stars of one plane. `plane` never changes, so the subtree is memoised and a camera
/// move never re-diffs the star nodes.
#[component]
fn StarField(plane: usize) -> Element {
    let spec = &PLANES[plane];
    let (width, height) = spec.box_size;
    let box_style = format!(
        "left:{:.1}%;top:{:.1}%;width:{width:.1}%;height:{height:.1}%",
        (100.0 - width) / 2.0,
        (100.0 - height) / 2.0,
    );
    let (class, style) = match &spec.drift {
        Some(drift) => (
            "sky-field drift",
            format!("{box_style};{}", drift_style(drift)),
        ),
        None => ("sky-field", box_style),
    };
    rsx! {
        div { class, style,
            for star in field()[plane].iter() {
                i { class: star.class, style: "{star.style}" }
            }
        }
    }
}

/// Gradient backdrop, drifting haze, three parallax star planes, the afterglow over the
/// stars nearest the horizon, and the vignette above everything.
///
/// Reads no navigation state: the camera arrives as `--pan` and `--depth` on an ancestor and
/// the planes resolve it in `calc()`.
#[component]
pub fn Sky() -> Element {
    rsx! {
        div { class: "sky", aria_hidden: "true",
            div { class: "sky-haze", style: parallax_style(HAZE_PARALLAX),
                div { class: "haze-field drift", style: drift_style(&HAZE_DRIFT),
                    div { class: "haze haze-a" }
                    div { class: "haze haze-b" }
                    div { class: "milkyway" }
                }
            }
            for (index , spec) in PLANES.iter().enumerate() {
                div { class: "sky-plane", style: parallax_style(spec.parallax),
                    StarField { plane: index }
                }
            }
            div { class: "sky-glow", style: parallax_style(GLOW_PARALLAX),
                div { class: "glow-field drift", style: drift_style(&GLOW_DRIFT),
                    div { class: "horizon" }
                    div { class: "afterglow" }
                }
            }
            div { class: "vignette" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seed_replays_the_same_sequence() {
        let (mut a, mut b) = (Rng::new(0x5EED_0A71), Rng::new(0x5EED_0A71));
        for _ in 0..256 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let (mut a, mut b) = (Rng::new(1), Rng::new(2));
        assert!((0..8).any(|_| a.next_u32() != b.next_u32()));
    }

    #[test]
    fn a_zero_seed_does_not_stick_at_zero() {
        let mut rng = Rng::new(0);
        assert!(
            (0..16).any(|_| rng.next_u32() != 0),
            "xorshift stays at zero forever unless the seed is forced odd"
        );
    }

    #[test]
    fn unit_stays_inside_the_unit_interval() {
        let mut rng = Rng::new(0x1234_5678);
        for _ in 0..20_000 {
            let u = rng.unit();
            assert!((0.0..1.0).contains(&u), "{u} outside [0, 1)");
        }
    }

    #[test]
    fn range_stays_within_its_bounds() {
        let mut rng = Rng::new(0x8765_4321);
        for spec in &PLANES {
            for _ in 0..2_000 {
                let size = rng.range(spec.size.0, spec.size.1);
                assert!((spec.size.0..=spec.size.1).contains(&size));
                let opacity = rng.range(spec.opacity.0, spec.opacity.1);
                assert!((spec.opacity.0..=spec.opacity.1).contains(&opacity));
            }
        }
    }

    #[test]
    fn index_stays_in_bounds() {
        let mut rng = Rng::new(99);
        for _ in 0..20_000 {
            assert!(rng.index(TINTS.len()) < TINTS.len());
        }
    }

    #[test]
    fn every_plane_generates_its_declared_count() {
        for spec in &PLANES {
            assert_eq!(generate(spec).len(), spec.count);
        }
    }

    #[test]
    fn the_field_is_built_once_and_matches_the_specs() {
        let first = field();
        assert!(std::ptr::eq(first, field()), "field is generated once");
        assert_eq!(first.len(), PLANES.len());
        for (plane, spec) in first.iter().zip(&PLANES) {
            assert_eq!(plane.len(), spec.count);
        }
    }

    #[test]
    fn planes_that_do_not_twinkle_produce_no_twinkling_stars() {
        for spec in PLANES.iter().filter(|s| s.twinkle_share <= 0.0) {
            assert!(generate(spec)
                .iter()
                .all(|star| !star.class.contains("twinkle")));
        }
    }

    #[test]
    fn flares_land_on_every_nth_star_and_nowhere_else() {
        for spec in &PLANES {
            let stars = generate(spec);
            for (i, star) in stars.iter().enumerate() {
                let expected = spec.flare_every > 0 && i % spec.flare_every == 0;
                assert_eq!(star.class.contains("flare"), expected, "star {i}");
            }
        }
    }

    #[test]
    fn every_star_carries_the_properties_the_stylesheet_reads() {
        for spec in &PLANES {
            for star in generate(spec) {
                for property in ["--tint:", "--o-dim:", "--o-bright:", "left:", "top:"] {
                    assert!(star.style.contains(property), "missing {property}");
                }
                assert_eq!(star.style.contains("box-shadow"), spec.glow);
            }
        }
    }

    #[test]
    fn styles_render_the_values_the_keyframes_expect() {
        assert_eq!(parallax_style(0.5), "--parallax:0.5");
        let drift = drift_style(&GLOW_DRIFT);
        assert!(drift.contains("--dx:4.00%"), "{drift}");
        assert!(drift.contains("--rot:0.00deg"), "{drift}");
        assert!(drift.contains("animation-duration:320s"), "{drift}");
    }

    // Reads a numeric value back out of a generated inline style.
    fn value(style: &str, key: &str) -> f32 {
        let start = style
            .find(key)
            .unwrap_or_else(|| panic!("`{key}` missing from `{style}`"))
            + key.len();
        let rest = &style[start..];
        let end = rest
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(rest.len());
        rest[..end].parse().expect("numeric value")
    }

    #[test]
    fn stars_stay_inside_their_box() {
        for spec in &PLANES {
            for star in generate(spec) {
                let (x, y) = (value(&star.style, "left:"), value(&star.style, "top:"));
                assert!((0.0..100.0).contains(&x), "left {x}% outside the box");
                assert!((0.0..100.0).contains(&y), "top {y}% outside the box");
            }
        }
    }

    #[test]
    fn stars_are_sized_within_their_plane_range() {
        for spec in &PLANES {
            for star in generate(spec) {
                let width = value(&star.style, "width:");
                assert!(
                    (spec.size.0..=spec.size.1).contains(&width),
                    "width {width}px"
                );
            }
        }
    }

    #[test]
    fn a_twinkle_always_brightens() {
        for spec in &PLANES {
            for star in generate(spec) {
                let dim = value(&star.style, "--o-dim:");
                let bright = value(&star.style, "--o-bright:");
                assert!(
                    dim <= bright,
                    "dim {dim} above bright {bright} would invert the twinkle"
                );
                assert!(
                    (0.0..=1.0).contains(&bright),
                    "opacity {bright} out of range"
                );
            }
        }
    }

    #[test]
    fn brightness_falls_off_toward_the_horizon() {
        let spec = &PLANES[2];
        let stars = generate(spec);
        let ceiling = |style: &str| {
            let y = value(style, "top:");
            spec.opacity.1 * (1.0 - 0.45 * (y / 100.0))
        };
        for star in &stars {
            let bright = value(&star.style, "--o-bright:");
            assert!(
                bright <= ceiling(&star.style) + 1e-3,
                "star below the horizon wash is too bright"
            );
        }
    }
}
