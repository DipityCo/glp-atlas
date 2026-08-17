//! Navigation state, and the camera coordinates it projects onto the star field.

use dioxus::prelude::*;

use crate::stock::StockId;
use crate::store::DoseId;

/// Top-level destinations, ordered left to right across the field.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Doses,
    Progress,
    Profile,
}

impl Page {
    pub const ALL: [Page; 3] = [Page::Doses, Page::Progress, Page::Profile];

    /// Position across the field, left to right.
    pub fn index(self) -> u8 {
        match self {
            Page::Doses => 0,
            Page::Progress => 1,
            Page::Profile => 2,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Page::Doses => "Doses",
            Page::Progress => "Progress",
            Page::Profile => "Profile",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Page::Doses => "Your weekly rhythm",
            Page::Progress => "Where the numbers are heading",
            Page::Profile => "Medication and preferences",
        }
    }
}

/// A step across the field, in the order [`Page::ALL`] declares.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Previous,
    Next,
}

/// A page stacked on top of a [`Page`].
///
/// The dose and inventory pages carry an id rather than a position in a list, so a stack left
/// standing while the records change underneath it still names what it was opened on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubPage {
    DoseDetail(DoseId),
    LogDose,
    EditDose(DoseId),
    Inventory,
    AddStock,
    StockDetail(StockId),
    EditStock(StockId),
    LogWeight,
    Measurements,
    Medication,
    Reminders,
}

impl SubPage {
    pub fn title(self) -> &'static str {
        match self {
            SubPage::DoseDetail(_) => "Dose",
            SubPage::LogDose => "Log a dose",
            SubPage::EditDose(_) => "Edit dose",
            SubPage::Inventory => "Inventory",
            SubPage::AddStock => "Add to inventory",
            SubPage::StockDetail(_) => "Vials",
            SubPage::EditStock(_) => "Edit vials",
            SubPage::LogWeight => "Log weight",
            SubPage::Measurements => "Measurements",
            SubPage::Medication => "Medication",
            SubPage::Reminders => "Reminders",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            SubPage::DoseDetail(_) => "What was taken, and when",
            SubPage::LogDose => "Confirm the details",
            SubPage::EditDose(_) => "Change what was logged",
            SubPage::Inventory => "What you have on hand",
            SubPage::AddStock => "Vials as they arrived",
            SubPage::StockDetail(_) => "Sealed, mixed, and what is left",
            SubPage::EditStock(_) => "Change what these are",
            SubPage::LogWeight => "Today's reading",
            SubPage::Measurements => "Waist, chest, hips",
            SubPage::Medication => "Drug, strength, schedule",
            SubPage::Reminders => "When Atlas nudges you",
        }
    }
}

/// Direction of the last navigation.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Motion {
    #[default]
    None,
    PanLeft,
    PanRight,
    ZoomIn,
    ZoomOut,
}

impl Motion {
    /// Shell classes carrying the matching enter keyframes.
    pub fn class(self) -> &'static str {
        match self {
            Motion::None => "shell",
            Motion::PanLeft => "shell enter-pan-left",
            Motion::PanRight => "shell enter-pan-right",
            Motion::ZoomIn => "shell enter-zoom-in",
            Motion::ZoomOut => "shell enter-zoom-out",
        }
    }
}

/// Where the camera sits in the field: `pan` in page units (0..2), `depth` in sub-page
/// steps.
#[derive(Clone, Copy, PartialEq)]
pub struct Camera {
    pub pan: f32,
    pub depth: f32,
}

/// Navigation state, shared through context.
///
/// The two axes are independent: pages sit side by side and are reached by swiping or by the
/// tab bar, while back moves up out of a sub-page and never across. Each page keeps its own
/// sub-page stack, so leaving a page and returning lands where it was left. The document's own
/// history is not involved; `MainActivity` drives back through the page, and finishes the
/// activity once [`Nav::can_go_back`] is false.
///
/// Every field is a [`Signal`], a copyable handle rather than copied state, so all copies reach
/// the same slots and handlers can take a `Nav` by value. An ordinary field would diverge per
/// copy.
#[derive(Clone, Copy)]
pub struct Nav {
    page: Signal<Page>,
    stacks: Signal<[Vec<SubPage>; Page::ALL.len()]>,
    motion: Signal<Motion>,
    /// Increments on every navigation. Content is keyed on it, so the shell remounts and
    /// replays its enter animation.
    step: Signal<u32>,
}

impl Nav {
    pub fn new() -> Self {
        Self {
            page: Signal::new(Page::Doses),
            stacks: Signal::new([const { Vec::new() }; Page::ALL.len()]),
            motion: Signal::new(Motion::None),
            step: Signal::new(0),
        }
    }

    pub fn page(self) -> Page {
        *self.page.read()
    }

    /// Index of the current page's sub-page stack.
    fn slot(self) -> usize {
        usize::from(self.page().index())
    }

    /// Every sub-page open anywhere. A page keeps its stack while another is looked at, so
    /// what is open is not only what is on screen.
    pub fn open(self) -> Vec<SubPage> {
        self.stacks.read().iter().flatten().copied().collect()
    }

    pub fn holds(self, sub: SubPage) -> bool {
        self.open().contains(&sub)
    }

    pub fn top(self) -> Option<SubPage> {
        self.stacks.read()[self.slot()].last().copied()
    }

    /// How many sub-pages stand on the current page. Saturates at [`u8::MAX`]; the stack grows
    /// only by user navigation and never approaches it.
    pub fn depth(self) -> u8 {
        u8::try_from(self.stacks.read()[self.slot()].len()).unwrap_or(u8::MAX)
    }

    pub fn motion(self) -> Motion {
        *self.motion.read()
    }

    pub fn step(self) -> u32 {
        *self.step.read()
    }

    /// The camera at rest. A swipe drives the field ahead of this until it commits.
    pub fn camera(self) -> Camera {
        Camera {
            pan: f32::from(self.page().index()),
            depth: f32::from(self.depth()),
        }
    }

    /// Whether a back gesture has somewhere to go inside the app.
    pub fn can_go_back(self) -> bool {
        self.depth() > 0
    }

    /// The page a step in this direction would land on, if there is one.
    pub fn neighbour(self, direction: Direction) -> Option<Page> {
        let index = self.page().index();
        let target = match direction {
            Direction::Next => index + 1,
            Direction::Previous => index.checked_sub(1)?,
        };
        Page::ALL.get(usize::from(target)).copied()
    }

    fn navigate(&mut self, motion: Motion) {
        self.motion.set(motion);
        *self.step.write() += 1;
    }

    /// Switches page. Both pages' sub-page stacks are left standing.
    pub fn go(&mut self, page: Page) {
        let from = self.page();
        if from == page {
            return;
        }
        self.page.set(page);
        self.navigate(if page.index() > from.index() {
            Motion::PanLeft
        } else {
            Motion::PanRight
        });
    }

    /// Pushes a sub-page onto the current page.
    pub fn push(&mut self, sub: SubPage) {
        let slot = self.slot();
        self.stacks.write()[slot].push(sub);
        self.navigate(Motion::ZoomIn);
    }

    /// Leaves the current page's top sub-page, or does nothing at its root.
    pub fn back(&mut self) {
        if self.depth() == 0 {
            return;
        }
        let slot = self.slot();
        self.stacks.write()[slot].pop();
        self.navigate(Motion::ZoomOut);
    }
}

impl Default for Nav {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Signals need a runtime and an owning scope; `Nav` is otherwise plain state.
    fn nav(f: impl FnOnce(Nav)) {
        let dom = VirtualDom::new(|| rsx! { div {} });
        dom.in_runtime(|| {
            dioxus::core::Runtime::current().in_scope(ScopeId::ROOT, || f(Nav::new()));
        });
    }

    #[test]
    fn starts_at_the_first_page_with_nothing_stacked() {
        nav(|nav| {
            assert_eq!(nav.page(), Page::Doses);
            assert_eq!(nav.depth(), 0);
            assert_eq!(nav.top(), None);
            assert_eq!(nav.step(), 0);
            assert!(matches!(nav.motion(), Motion::None));
            assert!(!nav.can_go_back());
        });
    }

    #[test]
    fn page_indices_match_their_order_in_all() {
        for (position, page) in Page::ALL.iter().enumerate() {
            assert_eq!(usize::from(page.index()), position);
        }
    }

    #[test]
    fn going_forward_and_back_across_pages_sets_the_matching_motion() {
        nav(|mut nav| {
            nav.go(Page::Profile);
            assert_eq!(nav.page(), Page::Profile);
            assert!(matches!(nav.motion(), Motion::PanLeft));
            assert_eq!(nav.step(), 1);

            nav.go(Page::Doses);
            assert!(matches!(nav.motion(), Motion::PanRight));
            assert_eq!(nav.step(), 2);
        });
    }

    #[test]
    fn going_to_the_current_page_changes_nothing() {
        nav(|mut nav| {
            nav.go(Page::Progress);
            let step = nav.step();

            nav.go(Page::Progress);
            assert_eq!(nav.step(), step);
            assert!(matches!(nav.motion(), Motion::PanLeft));
        });
    }

    #[test]
    fn pushing_and_leaving_a_sub_page_moves_the_camera_in_and_out() {
        nav(|mut nav| {
            nav.push(SubPage::LogDose);
            assert_eq!(nav.depth(), 1);
            assert_eq!(nav.top(), Some(SubPage::LogDose));
            assert!(matches!(nav.motion(), Motion::ZoomIn));
            assert!(nav.can_go_back());

            nav.back();
            assert_eq!(nav.depth(), 0);
            assert_eq!(nav.top(), None);
            assert!(matches!(nav.motion(), Motion::ZoomOut));
        });
    }

    #[test]
    fn back_at_a_page_root_does_nothing() {
        nav(|mut nav| {
            let step = nav.step();
            nav.back();
            assert_eq!(nav.step(), step);
            assert_eq!(nav.depth(), 0);
            assert!(matches!(nav.motion(), Motion::None));
        });
    }

    #[test]
    fn sub_pages_stack_and_unwind_one_at_a_time() {
        nav(|mut nav| {
            nav.push(SubPage::Medication);
            nav.push(SubPage::Reminders);
            assert_eq!(nav.depth(), 2);
            assert_eq!(nav.top(), Some(SubPage::Reminders));

            nav.back();
            assert_eq!(nav.top(), Some(SubPage::Medication));
        });
    }

    #[test]
    fn each_page_keeps_its_own_stack() {
        nav(|mut nav| {
            nav.push(SubPage::LogDose);
            nav.go(Page::Progress);
            assert_eq!(nav.depth(), 0, "a fresh page starts at its root");

            nav.push(SubPage::Measurements);
            nav.go(Page::Doses);
            assert_eq!(
                nav.top(),
                Some(SubPage::LogDose),
                "returning restores the stack"
            );

            nav.go(Page::Progress);
            assert_eq!(nav.top(), Some(SubPage::Measurements));
        });
    }

    #[test]
    fn back_never_crosses_between_pages() {
        nav(|mut nav| {
            nav.go(Page::Progress);
            nav.push(SubPage::LogWeight);
            nav.back();

            assert_eq!(nav.page(), Page::Progress);
            assert!(!nav.can_go_back());
        });
    }

    #[test]
    fn the_field_ends_at_the_outer_pages() {
        nav(|mut nav| {
            assert_eq!(nav.neighbour(Direction::Next), Some(Page::Progress));
            assert_eq!(
                nav.neighbour(Direction::Previous),
                None,
                "nothing left of the first page"
            );

            nav.go(Page::Profile);
            assert_eq!(
                nav.neighbour(Direction::Next),
                None,
                "nothing right of the last page"
            );
            assert_eq!(nav.neighbour(Direction::Previous), Some(Page::Progress));
        });
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn the_camera_tracks_page_and_stack_depth() {
        nav(|mut nav| {
            assert_eq!(nav.camera().pan, 0.0);
            assert_eq!(nav.camera().depth, 0.0);

            nav.go(Page::Profile);
            assert_eq!(nav.camera().pan, 2.0);

            nav.push(SubPage::Reminders);
            assert_eq!(nav.camera().depth, 1.0);
        });
    }

    #[test]
    fn copies_share_one_state() {
        nav(|nav| {
            let mut copy = nav;
            copy.push(SubPage::LogDose);
            assert_eq!(nav.depth(), 1, "a copy writes through to the same slots");
        });
    }

    #[test]
    fn every_motion_class_is_defined_in_the_stylesheet() {
        use crate::screens::STYLESHEET;

        for motion in [
            Motion::None,
            Motion::PanLeft,
            Motion::PanRight,
            Motion::ZoomIn,
            Motion::ZoomOut,
        ] {
            for class in motion.class().split_whitespace() {
                assert!(
                    STYLESHEET.contains(&format!(".{class}")),
                    "`{class}` has no rule in the stylesheet"
                );
                if class.starts_with("enter-") {
                    assert!(
                        STYLESHEET.contains(&format!("@keyframes {class}")),
                        "`{class}` names no keyframes"
                    );
                }
            }
        }
    }

    #[test]
    fn the_middle_page_has_a_neighbour_on_both_sides() {
        nav(|mut nav| {
            nav.go(Page::Progress);
            assert_eq!(nav.neighbour(Direction::Previous), Some(Page::Doses));
            assert_eq!(nav.neighbour(Direction::Next), Some(Page::Profile));
        });
    }

    #[test]
    fn all_three_stacks_stand_independently() {
        nav(|mut nav| {
            nav.push(SubPage::LogDose);
            nav.go(Page::Progress);
            nav.push(SubPage::Measurements);
            nav.go(Page::Profile);
            nav.push(SubPage::Reminders);

            for (page, expected) in [
                (Page::Doses, SubPage::LogDose),
                (Page::Progress, SubPage::Measurements),
                (Page::Profile, SubPage::Reminders),
            ] {
                nav.go(page);
                assert_eq!(nav.top(), Some(expected));
                assert_eq!(nav.depth(), 1);
            }
        });
    }

    #[test]
    fn sub_pages_carrying_different_payloads_stack_separately() {
        nav(|mut nav| {
            let first = DoseId::new(0);
            let second = DoseId::new(3);
            nav.push(SubPage::DoseDetail(first));
            nav.push(SubPage::DoseDetail(second));
            assert_eq!(nav.depth(), 2);
            assert_eq!(nav.top(), Some(SubPage::DoseDetail(second)));

            nav.back();
            assert_eq!(nav.top(), Some(SubPage::DoseDetail(first)));
        });
    }
}
