//! The drawing of the handheld screens: thin, since the models decide what is shown. Rows are
//! painted on the window's draw list rather than as widgets, because the selection belongs to the
//! models and the gamepad moves it; the focus is a bright bar with dark text. Each row and each
//! hint records where it lies, for the mouse of the desktop's console mode ([`super::pointer`]).

use std::cell::{Cell, RefCell};

use imgui::{Condition, DrawListMut, StyleColor, StyleVar, Ui, WindowFlags, WindowToken};
use zax_platform::input::labels;
use zax_settings::catalogue::SettingId;

use super::achievements::{AchievementsRow, SpecialKey, SPECIAL_KEYS};
use super::compat::Tone;
use super::game_options::{GameRow, GAME_MAIN};
use super::layout::{self, Hint, Metrics};
use super::options;
use super::overlay::OverlayItem;
use super::overscan;
use super::pointer::{self, Zone, ZoneAction};
use super::rom_list::{first_visible, ProfileMatch, RomEntry, RomListModel};
use super::states::{SlotState, StatesMode};
use super::{
    slot_state, HandheldUi, Screen, CART_PICTURE_ID, COVER_PICTURE_ID, SHOT_PICTURE_ID,
    STATE_PICTURE_ID,
};
use crate::achievements::KnownGame;
use crate::emulation::commands::Command;
use crate::input::BindingDeviceKind;
use crate::persistence::save_slots::numbered_slot_id;
use crate::ui::confirm_dialog::ConfirmationView;
use crate::ui::menu_bar;
use crate::ui::zax_ui::{UiFrame, ZaxUi};
use crate::ui::{key_config, keys, strings, text};

/// The track of a list's scrollbar.
const SCROLL_TRACK: [f32; 4] = [1.0, 1.0, 1.0, 0.10];
/// Its thumb: the rows shown.
const SCROLL_THUMB: [f32; 4] = [1.0, 1.0, 1.0, 0.45];
// Xnes3D+: the NES look of the mockups: charcoal screens, a light grey title band with the
// console's ridges, and a light grey focus bar with a red edge.
const BACKGROUND: [f32; 4] = [0.11, 0.11, 0.118, 1.0];
const PANEL: [f32; 4] = [0.11, 0.11, 0.118, 0.97];
const TEXT: [f32; 4] = [0.925, 0.925, 0.925, 1.0];
const DIM: [f32; 4] = [0.71, 0.71, 0.73, 1.0];
const FOCUS: [f32; 4] = [0.925, 0.925, 0.925, 1.0];
/// Xnes3D+: the title band, its text and its ridges.
const TITLE_BAND: [f32; 4] = [0.79, 0.79, 0.79, 1.0];
const TITLE_TEXT: [f32; 4] = [0.11, 0.11, 0.118, 1.0];
const TITLE_DIM: [f32; 4] = [0.24, 0.24, 0.26, 1.0];
const RIDGE: [f32; 4] = [0.61, 0.61, 0.61, 1.0];
/// Xnes3D+: the gold of an unlocked achievement (the focus bar is grey now) and the red of the
/// share of a game's achievements in the list.
const GOLD: [f32; 4] = [1.0, 0.82, 0.16, 1.0];
const PERCENT: [f32; 4] = [1.0, 0.35, 0.4, 1.0];
const ON_FOCUS: [f32; 4] = [0.04, 0.04, 0.04, 1.0];
const PROFILE: [f32; 4] = [0.45, 0.92, 0.55, 1.0];
const PROFILE_ON_FOCUS: [f32; 4] = [0.02, 0.32, 0.08, 1.0];
const BAR: [f32; 4] = [0.17, 0.17, 0.18, 1.0];
const WARNING: [f32; 4] = [1.0, 0.45, 0.35, 1.0];
/// The [W] of a game whose ROM was not studied yet: its profile or its name says a hack.
const UNSTUDIED: [f32; 4] = [0.42, 0.44, 0.5, 1.0];
/// Xnes3D+: the NES look. The red of the stripe under the title, and of the A and B button
/// badges of the hints; the grey of the other buttons; the line between rows.
const NES_RED: [f32; 4] = [0.85, 0.09, 0.15, 1.0];
const NES_BUTTON_GREY: [f32; 4] = [0.36, 0.36, 0.38, 1.0];
const ON_BUTTON: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const ROW_LINE: [f32; 4] = [1.0, 1.0, 1.0, 0.07];
/// The logo in the title band of the game list: the name, then the "+" in red.
const LOGO_NAME: &str = "XNES3D";
const LOGO_PLUS: &str = "+";
/// Xnes3D+: the 5 x 7 pixel letters of the logo, a row per byte, the left square the high
/// bit of five. Letters it does not have are blank.
const fn pixel_letter(letter: char) -> [u8; 7] {
    match letter {
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '+' => [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000],
        _ => [0; 7],
    }
}

/// What marks a favourite game in the home list (AMD-053 REQ-UI-210; the form is free).
const FAVORITE_MARKER: &str = "*";
/// Xnes3D+: what marks an option holding something other than its default.
const CHANGED_MARKER: &str = "changed";
/// A value a game has of its own, and the hint of the list when the game has some.
const GAME_OWN: [f32; 4] = [0.3, 0.9, 1.0, 1.0];
const GAME_OWN_ON_FOCUS: [f32; 4] = [0.0, 0.3, 0.4, 1.0];
/// RO-3 §2 rule 1: what marks a row holding the game's own value rather than the global one. Drawn
/// in [`GAME_OWN`], as a badge before the value, so that the value's own span — what a click on the
/// desktop's console mode steps — keeps its place.
const GAME_OWN_MARKER: &str = "*";
/// A group title of the option tree (RO-3): dimmer than [`DIM`], which is a value.
const GROUP_TITLE: [f32; 4] = [0.5, 0.54, 0.62, 1.0];
/// The save state screen when it saves, and when it loads: two colours apart from each other and
/// from the orange and red of the states that do not load as they are.
/// Xnes3D+: a light red and a light grey, in the NES colours, dark text reading on both.
const SAVE_ACCENT: [f32; 4] = [0.96, 0.6, 0.62, 1.0];
const LOAD_ACCENT: [f32; 4] = [0.8, 0.8, 0.82, 1.0];
/// A state made with the built-in patches set the other way (it loads once they are switched).
const OTHER_MODE: [f32; 4] = [1.0, 0.62, 0.2, 1.0];

/// The path of pages open over the main one, as the title band spells it out: "Advanced options >
/// 3D rendering > Enhanced effects". Empty on the main page (RO-3).
fn page_path(path: &[options::OptionsPage]) -> String {
    path.iter()
        .map(|page| options::page_title(*page))
        .collect::<Vec<_>>()
        .join(" > ")
}

/// A few games for the screenshots of a machine with no ROM folder.
pub(super) fn sample_roms() -> RomListModel {
    let entry = |name: &str, profile: Option<&str>| {
        let mut entry = RomEntry::new(std::path::PathBuf::from(format!("/roms/{name}.nes")));
        entry.profile = profile.map(|title| ProfileMatch {
            id: title.to_lowercase(),
            title: title.to_owned(),
            two_dimensional: false,
        });
        entry
    };
    RomListModel::new(vec![
        entry("Adventure Island", Some("Adventure Island")),
        entry("Brick Breaker", None),
        entry("Castle Quest (U)", Some("Castle Quest")),
        entry("Dungeon Dash", None),
        entry("Escape From Mars", Some("Escape From Mars")),
        entry("Forest Runner", None),
        entry("Galaxy Raiders", Some("Galaxy Raiders")),
        entry("Haunted Halls", None),
        entry("Ice Climber Jr", None),
        entry("Jungle Jump", Some("Jungle Jump")),
    ])
}

/// What the painting of one screen needs.
struct Painter<'ui> {
    ui: &'ui Ui,
    list: DrawListMut<'ui>,
    metrics: Metrics,
    size: [f32; 2],
    heading: Option<imgui::FontId>,
    /// Where the rows and the hints drawn lie.
    zones: &'ui RefCell<Vec<Zone>>,
    /// The widescreen button being held (owner request): its hint's label and how far the hold
    /// is to the widescreen's settings, 0 to 1, drawn as a ring filling round the label.
    hold_ring: Option<(&'static str, f32)>,
    /// How many lines the hints drawn took, for the sizes of the next frame.
    hint_lines: &'ui Cell<u8>,
    /// How much lower the next rows lie, for the rows drawn on two lines above them.
    lift: Cell<f32>,
    /// How far right the rows start: the room of the list's scrollbar, when it has one.
    inset: Cell<f32>,
    /// Xnes3D+: how far right the list starts: the room of the cover panel of the game list.
    shift: Cell<f32>,
    /// The rank of the first row of the list drawn (group titles not counted), and how many
    /// titles it drew so far: the rank of each row's zone.
    rank_base: Cell<usize>,
    titles: Cell<usize>,
    /// The rank of the selected row of the list drawn, for the clicks.
    focus_rank: &'ui Cell<Option<usize>>,
    /// The rows it shows and how many a selection can rest on, for Page Up and Page Down.
    list_span: &'ui Cell<(usize, usize)>,
    /// The first row the scrollbar, the wheel or a finger scrolled the list to; none while the
    /// list follows its selection (owner request).
    view_first: Option<usize>,
}

/// One row of a list, as a screen describes it before the painter lays the list out.
enum RowSpec {
    /// A group title (RO-3).
    Group(String),
    /// A label, badges and a value in its colour.
    Row {
        left: String,
        badges: Vec<(&'static str, [f32; 4])>,
        right: String,
        colour: [f32; 4],
    },
}

impl Painter<'_> {
    fn zone(&self, min: [f32; 2], max: [f32; 2], action: ZoneAction) {
        self.zones.borrow_mut().push(Zone { min, max, action });
    }

    fn text(&self, x: f32, y: f32, colour: [f32; 4], text: &str) {
        self.list.add_text([x, y], colour, text);
    }

    fn width(&self, text: &str) -> f32 {
        self.ui.calc_text_size(text)[0]
    }

    /// The title band: the heading on the left, `right` in the body face on the right.
    fn title(&self, title: &str, right: &str) {
        let metrics = &self.metrics;
        self.list
            .add_rect([0.0, 0.0], [self.size[0], metrics.title_band()], TITLE_BAND)
            .filled(true)
            .build();
        // Xnes3D+: a red stripe under the band, as on the console's front.
        let stripe = (metrics.padding * 0.3).max(2.0);
        self.list
            .add_rect(
                [0.0, metrics.title_band() - stripe],
                [self.size[0], metrics.title_band()],
                NES_RED,
            )
            .filled(true)
            .build();
        let title_end;
        {
            let _heading = self.heading.map(|font| self.ui.push_font(font));
            if title == text::PRODUCT_NAME {
                // The game list carries the logo, in the pixel letters of the app icon: the
                // name, then the "+" in red.
                let pixel = (metrics.heading / 7.0).floor().max(2.0);
                let top = (metrics.title_band() - stripe - pixel * 7.0) * 0.5;
                let x = self.pixel_word(metrics.padding, top, pixel, TITLE_TEXT, LOGO_NAME);
                let end = self.pixel_word(x + pixel * 2.0, top, pixel, NES_RED, LOGO_PLUS);
                title_end = end + metrics.padding * 1.5;
            } else {
                self.text(metrics.padding, metrics.padding, TITLE_TEXT, title);
                title_end = metrics.padding * 2.5 + self.ui.calc_text_size(title)[0];
            }
        }
        let mut ridges_end = self.size[0] - metrics.padding;
        if !right.is_empty() {
            let x = self.size[0] - metrics.padding - self.width(right);
            let y = (metrics.title_band() - metrics.font) * 0.5;
            self.text(x, y, TITLE_DIM, right);
            ridges_end = x - metrics.padding * 1.5;
        }
        // Xnes3D+: the ridges of the console's front, between the title and the right text.
        if ridges_end - title_end > metrics.padding * 3.0 {
            let band = metrics.title_band() - stripe;
            let height = (metrics.font * 0.9).round();
            let mut y = ((band - height) * 0.5).round();
            let bottom = y + height;
            let line = (metrics.padding * 0.25).max(2.0).round();
            while y + line <= bottom {
                self.list
                    .add_rect([title_end, y], [ridges_end, y + line], RIDGE)
                    .filled(true)
                    .build();
                y += line * 2.3;
            }
        }
    }

    /// The top of the row `slot` of a list that starts under the title.
    fn row_top(&self, slot: usize) -> f32 {
        let slots = f32::from(u16::try_from(slot).unwrap_or(u16::MAX));
        self.metrics.title_band()
            + self.metrics.padding * 0.5
            + slots * self.metrics.row
            + self.lift.get()
    }

    /// The step from a label to its value set under it (owner request): a line of text, closer
    /// than the next row.
    fn wrap_step(&self) -> f32 {
        (self.metrics.font + self.metrics.padding * 0.3).round()
    }

    /// Whether `left`, its `badges` and `right` overflow a row from `x0` to `x1`: the value then
    /// goes under the label.
    fn wraps(
        &self,
        left: &str,
        badges: &[(&str, [f32; 4])],
        right: &str,
        (x0, x1): (f32, f32),
    ) -> bool {
        if left.is_empty() || right.is_empty() {
            return false;
        }
        let padding = self.metrics.padding;
        let badges: f32 = badges
            .iter()
            .map(|(badge, _)| self.width(badge) + padding)
            .sum();
        x0 + padding + self.width(left) + padding + badges + self.width(right) + padding > x1
    }

    /// The height of a row, on one line or on two.
    fn row_height(&self, wrapped: bool) -> f32 {
        self.metrics.row + if wrapped { self.wrap_step() } else { 0.0 }
    }

    /// The first row of a list of `total` rows, `rows` of them shown, its selection `selected`:
    /// where the scrollbar put it, else round the selection. Draws the scrollbar, and records
    /// the ranks of the rows the list draws.
    fn window(&self, selected: usize, rows: usize, total: usize) -> usize {
        let first = self.view_first.map_or_else(
            || first_visible(selected, rows, total),
            |first| first.min(total.saturating_sub(rows)),
        );
        self.rank_base.set(first);
        self.titles.set(0);
        self.focus_rank.set(Some(selected));
        self.list_span.set((rows, total));
        self.scrollbar(first, rows, total);
        first
    }

    /// The scrollbar of a list longer than the screen, on its left (owner request): the track
    /// over the list's height and the thumb of the rows shown. The rows start after it.
    fn scrollbar(&self, first: usize, rows: usize, total: usize) {
        if rows == 0 || total <= rows {
            self.inset.set(0.0);
            return;
        }
        let metrics = &self.metrics;
        // Four times the first width (owner request): wide enough for a finger.
        let width = (metrics.padding * 2.0).max(24.0).round();
        let left = (metrics.padding * 0.25).round() + self.shift.get();
        let top = metrics.title_band() + metrics.padding * 0.5;
        let room = self.list_room();
        let bottom = top + room;
        self.list
            .add_rect([left, top], [left + width, bottom], SCROLL_TRACK)
            .filled(true)
            .rounding(width * 0.5)
            .build();
        #[allow(clippy::cast_precision_loss)] // A list of a few thousand rows at most.
        let (thumb, offset) = {
            let thumb = (room * rows as f32 / total as f32).max(room * 0.05);
            let hidden = (total - rows) as f32;
            (
                thumb,
                (room - thumb) * first.min(total - rows) as f32 / hidden,
            )
        };
        self.list
            .add_rect(
                [left, top + offset],
                [left + width, top + offset + thumb],
                SCROLL_THUMB,
            )
            .filled(true)
            .rounding(width * 0.5)
            .build();
        let gap = (metrics.padding * 0.25).round();
        self.zone(
            [left - gap, top],
            [left + width + gap, bottom],
            ZoneAction::Scrollbar { first, rows, total },
        );
        self.inset.set(width + gap * 2.0);
    }

    /// The pixels a list has between the title and the hints.
    fn list_room(&self) -> f32 {
        self.size[1] - self.metrics.title_band() - self.metrics.hint_band() - self.metrics.padding
    }

    /// The height `spec` takes on a full-width row.
    fn spec_height(&self, spec: &RowSpec) -> f32 {
        match spec {
            RowSpec::Group(_) => self.metrics.row,
            RowSpec::Row {
                left,
                badges,
                right,
                ..
            } => {
                let span = (
                    self.metrics.padding * 0.5,
                    self.size[0] - self.metrics.padding * 0.5,
                );
                self.row_height(self.wraps(left, badges, right, span))
            }
        }
    }

    /// Draws the window of `specs` that shows `selected`, in `room` pixels (the whole list's room
    /// unless something takes some of it), rows of two lines included.
    fn spec_list(&self, specs: &[RowSpec], selected: usize, room: f32) {
        let heights: Vec<f32> = specs.iter().map(|spec| self.spec_height(spec)).collect();
        let rows = self.metrics.visible_rows(self.size[1]);
        // Scrolled (owner request): the rows from there, the end of the list kept full.
        let (first, count) = if let Some(first) = self.view_first {
            let first = first.min(specs.len().saturating_sub(1));
            layout::list_window(&heights, first, room, first)
        } else {
            let start = first_visible(selected, rows, specs.len());
            layout::list_window(&heights, selected, room, start)
        };
        let titles_before = |index: usize| {
            specs[..index.min(specs.len())]
                .iter()
                .filter(|spec| matches!(spec, RowSpec::Group(_)))
                .count()
        };
        self.rank_base.set(first - titles_before(first));
        self.titles.set(0);
        self.focus_rank
            .set(Some(selected.saturating_sub(titles_before(selected))));
        self.list_span
            .set((count, specs.len() - titles_before(specs.len())));
        self.scrollbar(first, count, specs.len());
        let x1 = self.size[0] - self.metrics.padding * 0.5;
        for (slot, (index, spec)) in specs.iter().enumerate().skip(first).take(count).enumerate() {
            match spec {
                RowSpec::Group(title) => self.group_row(slot, title),
                RowSpec::Row {
                    left,
                    badges,
                    right,
                    colour,
                } => self.row_parts(
                    slot,
                    (index == selected, FOCUS),
                    left,
                    badges,
                    (right, *colour),
                    x1,
                ),
            }
        }
    }

    /// One row of a list: `left` clipped to leave room for `right`.
    fn row(&self, slot: usize, selected: bool, left: &str, right: (&str, [f32; 4])) {
        let x1 = self.size[0] - self.metrics.padding * 0.5;
        self.row_to(slot, (selected, FOCUS), left, right, x1);
    }

    /// One row of a list that ends at `x1`, its focus bar of `focus` when selected.
    fn row_to(
        &self,
        slot: usize,
        (selected, focus): (bool, [f32; 4]),
        left: &str,
        right: (&str, [f32; 4]),
        x1: f32,
    ) {
        self.row_parts(slot, (selected, focus), left, &[], right, x1);
    }

    /// One row of a list with `badges` before `right`, left to right (the [W] of a game a
    /// widescreen hack is made for, the share of its achievements the player has).
    fn row_badged(
        &self,
        slot: usize,
        selected: bool,
        left: &str,
        badges: &[(&str, [f32; 4])],
        right: (&str, [f32; 4]),
    ) {
        let x1 = self.size[0] - self.metrics.padding * 0.5;
        self.row_parts(slot, (selected, FOCUS), left, badges, right, x1);
    }

    /// One row of a list that ends at `x1`: `left`, then `badges`, then `right`.
    fn row_parts(
        &self,
        slot: usize,
        (selected, focus): (bool, [f32; 4]),
        left: &str,
        badges: &[(&str, [f32; 4])],
        right: (&str, [f32; 4]),
        x1: f32,
    ) {
        let metrics = &self.metrics;
        let top = self.row_top(slot);
        let x0 = metrics.padding * 0.5 + self.inset.get() + self.shift.get();
        // Owner request: a value that does not fit beside its label goes under it.
        let wrapped = self.wraps(left, badges, right.0, (x0, x1));
        let height = self.row_height(wrapped);
        if wrapped {
            self.lift.set(self.lift.get() + self.wrap_step());
        }
        if selected {
            // Square corners: the pixel look of the fork.
            self.list
                .add_rect([x0, top], [x1, top + height], focus)
                .filled(true)
                .build();
            // Xnes3D+: the red edge of the focus bar.
            if focus == FOCUS {
                self.list
                    .add_rect([x0, top], [x0 + metrics.padding * 0.4, top + height], NES_RED)
                    .filled(true)
                    .build();
            }
        } else {
            self.list
                .add_line(
                    [x0 + metrics.padding, top + metrics.row - 1.0],
                    [x1 - metrics.padding, top + metrics.row - 1.0],
                    ROW_LINE,
                )
                .build();
        }
        let y = top + (metrics.row - metrics.font) * 0.5;
        // Grey on the bright bar would not read: the focus keeps dark text.
        let (main, side) = if selected {
            (ON_FOCUS, if right.1 == DIM { ON_FOCUS } else { right.1 })
        } else {
            (TEXT, right.1)
        };
        let right_width = self.width(right.0);
        let (right_x, right_y) = if wrapped {
            (x0 + metrics.padding * 2.0, y + self.wrap_step())
        } else {
            (x1 - metrics.padding - right_width, y)
        };
        if !right.0.is_empty() {
            self.text(right_x, right_y, side, right.0);
        }
        let value_end = if wrapped {
            right_x + right_width + metrics.padding
        } else {
            x1
        };
        let value = right.0.starts_with('<').then_some((right_x, value_end));
        self.zone(
            [x0, top],
            [x1, top + height],
            ZoneAction::Row {
                rank: (self.rank_base.get() + slot).saturating_sub(self.titles.get()),
                selected,
                value,
            },
        );
        let mut clip_to = if right.0.is_empty() || wrapped {
            x1
        } else {
            right_x - metrics.padding
        };
        for &(badge, colour) in badges.iter().rev() {
            let badge_x = clip_to - self.width(badge);
            // On the bright bar the badge sits on a dark pill, so it keeps its own colour.
            if selected {
                let pad = metrics.padding * 0.3;
                self.list
                    .add_rect(
                        [badge_x - pad, y - pad * 0.5],
                        [
                            badge_x + self.width(badge) + pad,
                            y + metrics.font + pad * 0.5,
                        ],
                        BAR,
                    )
                    .filled(true)
                    .rounding(pad)
                    .build();
            }
            self.text(badge_x, y, colour, badge);
            clip_to = badge_x - metrics.padding;
        }
        self.list
            .with_clip_rect_intersect([x0, top], [clip_to, top + metrics.row], || {
                self.text(x0 + metrics.padding, y, main, left);
            });
    }

    /// A group title (RO-3): a label and nothing else, drawn smaller and dimmed. It records **no**
    /// zone, so the mouse of the desktop's console mode cannot select what the pad cannot.
    fn group_row(&self, slot: usize, title: &str) {
        const SCALE: f32 = 0.8;
        let metrics = &self.metrics;
        let top = self.row_top(slot);
        self.titles.set(self.titles.get() + 1);
        self.ui.set_window_font_scale(SCALE);
        let y = top + (metrics.row - metrics.font * SCALE) * 0.6;
        self.text(
            metrics.padding * 1.5 + self.inset.get() + self.shift.get(),
            y,
            GROUP_TITLE,
            title,
        );
        self.ui.set_window_font_scale(1.0);
    }

    /// The bar of hints at the bottom.
    fn hints(&self, hints: &[Hint]) {
        let texts: Vec<(&str, String)> = hints
            .iter()
            .map(|hint| (hint.button, strings::get(hint.meaning)))
            .collect();
        self.hint_texts(&texts);
    }

    /// The bar of hints at the bottom, each a button and what it does, already in words.
    fn hint_texts(&self, hints: &[(&str, String)]) {
        let entries: Vec<(&str, String, [f32; 4])> = hints
            .iter()
            .map(|(button, meaning)| (*button, meaning.clone(), TEXT))
            .collect();
        self.hint_entries(&entries);
    }

    /// The same, each meaning in a colour of its own.
    /// Hints that do not fit the width go on a line under the others (owner request); the band
    /// grows upwards and the next frame's rows make room for it.
    fn hint_entries(&self, hints: &[(&str, String, [f32; 4])]) {
        let mut metrics = self.metrics;
        let gap = metrics.padding * 1.6;
        let room = self.size[0] - metrics.padding;
        let mut lines: Vec<usize> = Vec::new();
        let mut used = metrics.padding;
        for (index, (button, meaning, _)) in hints.iter().enumerate() {
            // Xnes3D+: the button is a badge, wider than its letters.
            let wide = self.badge_width(button) + metrics.padding * 0.5 + self.width(meaning);
            if index > 0 && used + wide > room {
                lines.push(index);
                used = metrics.padding;
            }
            used += wide + gap;
        }
        let count = u8::try_from(lines.len() + 1).unwrap_or(u8::MAX);
        self.hint_lines.set(count);
        metrics.hint_lines = count;
        let top = self.size[1] - metrics.hint_band();
        self.list
            .add_rect([0.0, top], self.size, BAR)
            .filled(true)
            .build();
        let mut y = top + metrics.padding;
        let mut x = metrics.padding;
        let mut band_top = top;
        for (index, (button, meaning, colour)) in hints.iter().enumerate() {
            if lines.contains(&index) {
                y += metrics.hint_line_step();
                x = metrics.padding;
                band_top = y - metrics.padding * 0.5;
            }
            let start = x;
            if let Some((_, progress)) = self.hold_ring.filter(|(label, _)| label == button) {
                self.ring(x, y, button, progress);
            }
            // Xnes3D+: the button as a badge, round for a face button, red for A and B as
            // on the NES pad, grey for the others.
            let badge = self.button_badge(x, y, button);
            x += badge + metrics.padding * 0.5;
            self.text(x, y, *colour, meaning);
            x += self.width(meaning) + metrics.padding * 1.6;
            if let Some(action) = pointer::hint_action(button) {
                let half_gap = metrics.padding * 0.8;
                let band_bottom = if lines.iter().any(|&next| next > index) {
                    y + metrics.font + metrics.padding * 0.25
                } else {
                    self.size[1]
                };
                self.zone(
                    [start - half_gap, band_top],
                    [x - half_gap, band_bottom],
                    action,
                );
            }
        }
    }

    /// Xnes3D+: `word` in 5 x 7 pixel letters (those of the logo, upper case only) whose
    /// squares are `pixel` wide, from `x`, `y`. Returns where the next letter would start.
    #[allow(clippy::cast_precision_loss)] // at most 7 rows of 5 squares
    fn pixel_word(&self, x: f32, y: f32, pixel: f32, colour: [f32; 4], word: &str) -> f32 {
        let mut x = x;
        for letter in word.chars() {
            for (row, bits) in pixel_letter(letter).iter().enumerate() {
                for column in 0..5_u8 {
                    if bits & (0b10000 >> column) != 0 {
                        let left = x + f32::from(column) * pixel;
                        let top = y + row as f32 * pixel;
                        self.list
                            .add_rect([left, top], [left + pixel, top + pixel], colour)
                            .filled(true)
                            .build();
                    }
                }
            }
            x += pixel * 6.0;
        }
        x - pixel
    }

    /// Xnes3D+: how wide the badge of `button` is.
    fn badge_width(&self, button: &str) -> f32 {
        let metrics = &self.metrics;
        let height = metrics.font + metrics.padding * 0.4;
        (self.width(button) + metrics.padding * 0.8).max(height)
    }

    /// Xnes3D+: `button` drawn as a badge whose left edge is at `x` and whose text sits on
    /// the line `y`. Returns its width.
    fn button_badge(&self, x: f32, y: f32, button: &str) -> f32 {
        let metrics = &self.metrics;
        let text_width = self.width(button);
        let height = metrics.font + metrics.padding * 0.4;
        let width = self.badge_width(button);
        let top = y - metrics.padding * 0.2;
        let colour = if matches!(button, "A" | "B") {
            NES_RED
        } else {
            NES_BUTTON_GREY
        };
        self.list
            .add_rect([x, top], [x + width, top + height], colour)
            .filled(true)
            .rounding(height * 0.5)
            .build();
        self.text(x + (width - text_width) * 0.5, y, ON_BUTTON, button);
        width
    }

    /// A ring round the label `button` drawn at `x`, `y`, filled clockwise from the top to
    /// `progress` (0 to 1): a held button's way to its long press.
    fn ring(&self, x: f32, y: f32, button: &str, progress: f32) {
        const SEGMENTS: f32 = 40.0;
        let width = self.width(button);
        let height = self.ui.text_line_height();
        let centre = [x + width * 0.5, y + height * 0.5];
        let radius = width.max(height) * 0.5 + self.metrics.padding * 0.35;
        // At least two points, a short arc at the start of the hold.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // 1 to 40.
        let steps = (SEGMENTS * progress).ceil().max(1.0) as usize;
        let points: Vec<[f32; 2]> = (0..=steps)
            .map(|step| {
                #[allow(clippy::cast_precision_loss)] // at most 40
                let angle = -std::f32::consts::FRAC_PI_2
                    + std::f32::consts::TAU * progress * (step as f32 / steps as f32);
                [
                    centre[0] + radius * angle.cos(),
                    centre[1] + radius * angle.sin(),
                ]
            })
            .collect();
        self.list
            .add_polyline(points, NES_RED)
            .thickness(self.metrics.padding * 0.18)
            .build();
    }

    /// Xnes3D+: `help` in a box just above the hints, the width of the screen. Returns the
    /// height it takes, which the list above leaves free.
    fn help_box(&self, help: &str) -> f32 {
        let metrics = &self.metrics;
        let left = metrics.padding * 0.5;
        let right = self.size[0] - metrics.padding * 0.5;
        let wrap = right - left - metrics.padding * 2.0;
        let text_height = self.ui.calc_text_size_with_opts(help, false, wrap)[1];
        let height = text_height + metrics.padding * 1.5;
        let bottom = self.size[1] - metrics.hint_band() - metrics.padding * 0.25;
        let top = bottom - height;
        self.list
            .add_rect([left, top], [right, bottom], BAR)
            .filled(true)
            .build();
        self.list
            .add_rect([left, top], [left + metrics.padding * 0.3, bottom], NES_RED)
            .filled(true)
            .build();
        self.ui
            .set_cursor_pos([left + metrics.padding, top + metrics.padding * 0.75]);
        let _wrap = self.ui.push_text_wrap_pos_with_pos(left + metrics.padding + wrap);
        let _colour = self.ui.push_style_color(StyleColor::Text, DIM);
        self.ui.text_wrapped(help);
        height + metrics.padding * 0.25
    }

    /// A paragraph in small print under the row `slot` of the list area.
    fn note(&self, slot: usize, text: &str) {
        let metrics = &self.metrics;
        let width = self.size[0] - 4.0 * metrics.padding;
        self.ui
            .set_cursor_pos([2.0 * metrics.padding, self.row_top(slot) + metrics.padding]);
        let _wrap = self
            .ui
            .push_text_wrap_pos_with_pos(2.0 * metrics.padding + width);
        let _colour = self.ui.push_style_color(StyleColor::Text, DIM);
        self.ui.text_wrapped(text);
    }

    /// Paragraphs one under the other from the top of the list area, each in its colour.
    fn lines(&self, lines: &[(String, [f32; 4])]) {
        let metrics = &self.metrics;
        let width = self.size[0] - 4.0 * metrics.padding;
        self.ui
            .set_cursor_pos([2.0 * metrics.padding, self.row_top(0) + metrics.padding]);
        let _wrap = self
            .ui
            .push_text_wrap_pos_with_pos(2.0 * metrics.padding + width);
        for (text, colour) in lines {
            let y = self.ui.cursor_pos()[1];
            self.ui.set_cursor_pos([2.0 * metrics.padding, y]);
            let _colour = self.ui.push_style_color(StyleColor::Text, *colour);
            self.ui.text_wrapped(text);
        }
    }

    /// A paragraph in the middle of the list area.
    fn message(&self, text: &str, colour: [f32; 4]) {
        let metrics = &self.metrics;
        let width = self.size[0] - 4.0 * metrics.padding;
        self.ui
            .set_cursor_pos([2.0 * metrics.padding, self.row_top(1)]);
        let _wrap = self
            .ui
            .push_text_wrap_pos_with_pos(2.0 * metrics.padding + width);
        let _colour = self.ui.push_style_color(StyleColor::Text, colour);
        self.ui.text_wrapped(text);
    }
}

/// A window over the whole display, with no frame, no scrolling and no focus of its own.
/// The share of the achievements of the game in `path` the player has, as the list studied it
/// (owner request: RetroAchievements on).
fn achievement_percent(frame: &UiFrame<'_>, path: &std::path::Path) -> Option<String> {
    frame
        .host
        .achievements
        .known_game(path)
        .flatten()
        .and_then(KnownGame::percent)
        .map(|percent| crate::ui::achievements_view::percent_text(percent, 100))
}

/// The achievement selected in the grid, told at `y`: its title and points, its description.
fn draw_grid_choice(
    painter: &Painter<'_>,
    achievement: &crate::achievements::KnownAchievement,
    y: f32,
) {
    let metrics = &painter.metrics;
    let points = strings::format(
        &strings::get(keys::ACH_POINTS),
        &[&achievement.points.to_string()],
    );
    let colour = if achievement.unlocked { GOLD } else { TEXT };
    painter.text(
        metrics.padding,
        y,
        colour,
        &format!("{}  {points}", achievement.title),
    );
    painter.text(
        metrics.padding,
        y + metrics.font * 1.2,
        DIM,
        &achievement.description,
    );
}

fn full_screen<'ui>(ui: &'ui Ui, name: &str, background: [f32; 4]) -> Option<WindowToken<'ui>> {
    full_screen_with(
        ui,
        name,
        background,
        WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS,
    )
}

/// The same, over every other window: Dear `ImGui` files a window that is not brought to the
/// front behind all the others when it first appears, which would hide a question.
fn full_screen_over<'ui>(
    ui: &'ui Ui,
    name: &str,
    background: [f32; 4],
) -> Option<WindowToken<'ui>> {
    full_screen_with(ui, name, background, WindowFlags::empty())
}

fn full_screen_with<'ui>(
    ui: &'ui Ui,
    name: &str,
    background: [f32; 4],
    extra: WindowFlags,
) -> Option<WindowToken<'ui>> {
    let _padding = ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0]));
    let _border = ui.push_style_var(StyleVar::WindowBorderSize(0.0));
    let _rounding = ui.push_style_var(StyleVar::WindowRounding(0.0));
    let _background = ui.push_style_color(StyleColor::WindowBg, background);
    ui.window(name)
        .position([0.0, 0.0], Condition::Always)
        .size(ui.io().display_size, Condition::Always)
        .flags(
            WindowFlags::NO_DECORATION
                | WindowFlags::NO_MOVE
                | WindowFlags::NO_SAVED_SETTINGS
                | WindowFlags::NO_NAV
                | extra,
        )
        .begin()
}

impl HandheldUi {
    /// Draws the screen in front, the hold of the exit combination and the notification.
    pub fn draw(&mut self, ui: &Ui, frame: &UiFrame<'_>) {
        self.zones.get_mut().clear();
        self.focus_rank.set(None);
        self.list_span.set((0, 0));
        let size = ui.io().display_size;
        // By the shorter side: the sizes of the landscape in portrait (owner request), which are
        // the sizes the fonts were baked at.
        self.metrics = self.metrics_for(size[0].min(size[1]));
        self.metrics.hint_lines = self.hint_lines.get();
        let _body = self.body_font().map(|font| ui.push_font(font));
        self.follow_the_state_picture(frame.host);
        self.follow_the_media(frame.host);
        match self.screen {
            Screen::Home => self.draw_home(ui, frame),
            Screen::GameOptions => self.draw_game_options(ui, frame),
            Screen::Hacks => self.draw_hacks(ui),
            Screen::Profiles => self.draw_profiles(ui),
            Screen::Options => self.draw_options(ui, frame),
            Screen::Controls => self.draw_controls(ui, frame),
            Screen::Overscan => self.draw_overscan(ui, frame),
            Screen::Overlay => self.draw_overlay(ui, frame),
            Screen::GameInfo => self.draw_game_info(ui, frame),
            Screen::States => self.draw_states(ui, frame),
            Screen::Achievements => self.draw_achievements(ui, frame),
            Screen::Keyboard => self.draw_keyboard(ui),
            Screen::AchievementList => self.draw_achievement_list(ui, frame),
            Screen::AchievementGrid => self.draw_achievement_grid(ui, frame),
            Screen::InGame => {}
        }
        if let Some(question) = ZaxUi::confirmation(frame.host) {
            self.draw_confirmation(ui, question);
        }
        self.draw_exit_hold(ui);
        self.draw_launch_animation(ui);
        let margin = self.metrics.hint_band() + self.metrics.padding;
        ZaxUi::draw_notification(ui, frame.host, margin);
    }

    /// Xnes3D+: the cartridge sliding into a grey console, its power light coming on, the name
    /// of the game under it, over everything for a moment when a game starts; then it fades.
    fn draw_launch_animation(&mut self, ui: &Ui) {
        const SECONDS: f32 = 1.1;
        const FADE: f32 = 0.3;
        let Some((name, t)) = self.launch_animation.as_mut().map(|(name, elapsed)| {
            *elapsed += ui.io().delta_time.min(0.1);
            (name.clone(), *elapsed)
        }) else {
            return;
        };
        if t >= SECONDS {
            self.launch_animation = None;
            return;
        }
        let alpha = if t > SECONDS - FADE { (SECONDS - t) / FADE } else { 1.0 };
        let fade = |colour: [f32; 4]| [colour[0], colour[1], colour[2], colour[3] * alpha];
        let list = ui.get_foreground_draw_list();
        let size = ui.io().display_size;
        let metrics = self.metrics;
        list.add_rect([0.0, 0.0], size, fade([0.07, 0.07, 0.08, 1.0]))
            .filled(true)
            .build();
        // The console, a little under the middle; its slot on top.
        let width = (size[0] * 0.42).min(size[1] * 0.9);
        let height = width * 0.36;
        let left = (size[0] - width) * 0.5;
        let top = size[1] * 0.5 - height * 0.2;
        let cart_width = width * 0.48;
        let cart_height = width * 0.36;
        let cart_left = (size[0] - cart_width) * 0.5;
        // The cartridge comes down over the first 60 % of the time, with a small bounce.
        let travel = (t / (SECONDS * 0.6)).min(1.0);
        let eased = 1.0 - (1.0 - travel) * (1.0 - travel);
        let bounce = if travel >= 1.0 { 0.0 } else { (travel * std::f32::consts::PI).sin() * cart_height * 0.03 };
        let cart_bottom_in = top + height * 0.15;
        let cart_top = cart_bottom_in - cart_height + (1.0 - eased) * -cart_height * 0.9 + bounce;
        list.add_rect([cart_left, cart_top], [cart_left + cart_width, cart_bottom_in], fade([0.56, 0.56, 0.58, 1.0]))
            .filled(true)
            .build();
        let label_pad = cart_width * 0.1;
        list.add_rect(
            [cart_left + label_pad, cart_top + label_pad],
            [cart_left + cart_width - label_pad, cart_top + cart_height * 0.68],
            fade([0.05, 0.05, 0.06, 1.0]),
        )
        .filled(true)
        .build();
        list.add_rect([left, top], [left + width, top + height], fade(TITLE_BAND))
            .filled(true)
            .build();
        list.add_rect([left, top + height], [left + width, top + height + metrics.padding * 0.8], fade(BAR))
            .filled(true)
            .build();
        // The slot, then the console's ridges.
        list.add_rect(
            [cart_left - width * 0.03, top],
            [cart_left + cart_width + width * 0.03, top + height * 0.12],
            fade(BACKGROUND),
        )
        .filled(true)
        .build();
        let line = (height * 0.035).max(2.0);
        let mut y = top + height * 0.3;
        while y + line < top + height * 0.55 {
            list.add_rect([left + width * 0.06, y], [left + width * 0.94, y + line], fade(RIDGE))
                .filled(true)
                .build();
            y += line * 2.3;
        }
        // POWER and RESET, and the light that comes on once the cartridge is in.
        let button_top = top + height * 0.66;
        let button_height = height * 0.18;
        for index in 0..2 {
            #[allow(clippy::cast_precision_loss)] // two buttons
            let x = left + width * 0.08 + index as f32 * width * 0.2;
            list.add_rect([x, button_top], [x + width * 0.16, button_top + button_height], fade(BAR))
                .filled(true)
                .build();
        }
        let led = if travel >= 1.0 { NES_RED } else { [0.3, 0.08, 0.1, 1.0] };
        list.add_circle([left + width * 0.5, button_top + button_height * 0.5], height * 0.05, fade(led))
            .filled(true)
            .build();
        // The name of the game under the console.
        let name_width = ui.calc_text_size(&name)[0];
        list.add_text(
            [(size[0] - name_width) * 0.5, top + height + metrics.padding * 2.0],
            fade(TEXT),
            &name,
        );
    }

    fn painter<'ui>(&'ui self, ui: &'ui Ui) -> Painter<'ui> {
        Painter {
            ui,
            list: ui.get_window_draw_list(),
            metrics: self.metrics,
            size: ui.io().display_size,
            heading: self.heading_font,
            zones: &self.zones,
            hint_lines: &self.hint_lines,
            lift: Cell::new(0.0),
            inset: Cell::new(0.0),
            shift: Cell::new(0.0),
            rank_base: Cell::new(0),
            titles: Cell::new(0),
            focus_rank: &self.focus_rank,
            list_span: &self.list_span,
            view_first: self.scroll.first,
            hold_ring: self
                .widescreen_hold
                .as_ref()
                .filter(|hold| hold.held > 0.0)
                // Only A is held for the widescreen now: L1 on the list opens its settings.
                .map(|hold| ("A", (hold.held / super::WIDESCREEN_HOLD_SECONDS).min(1.0))),
        }
    }

    #[allow(clippy::too_many_lines)] // One row per game, its badges in their order.
    fn draw_home(&self, ui: &Ui, frame: &UiFrame<'_>) {
        let Some(_window) = full_screen(ui, "##home", BACKGROUND) else {
            return;
        };
        let painter = self.painter(ui);
        let total = self.roms.entries().len();
        let position = if total == 0 {
            String::new()
        } else {
            let initial = self.roms.selected_entry().map_or('#', RomEntry::initial);
            format!("{initial}  {}/{total}", self.roms.selected() + 1)
        };
        painter.title(text::PRODUCT_NAME, &position);
        if total == 0 {
            let folder = self.roms_directory.display().to_string();
            // Xnes3D+: on Android the folder is chosen, not copied to.
            let message = if crate::android_bridge::available() {
                "No games here yet.\n\nPress A to choose the folder your NES games are in. \
                 You can change it later in Options > Games and 3D profiles."
                    .to_owned()
            } else {
                strings::format(&strings::get(keys::HANDHELD_ROMS_EMPTY), &[&folder])
            };
            painter.message(&message, TEXT);
        }
        // Xnes3D+: the cover of the selected game on the left, the list on the right.
        if total > 0 {
            let shelf = self.shelf_width(painter.size);
            if shelf > 0.0 {
                self.draw_cover_panel(&painter, shelf);
                painter.shift.set(shelf);
            }
        }
        let rows = self.metrics.visible_rows(painter.size[1]);
        let first = painter.window(self.roms.selected(), rows, total);
        for (slot, (index, entry)) in self
            .roms
            .entries()
            .iter()
            .enumerate()
            .skip(first)
            .take(rows)
            .enumerate()
        {
            let selected = index == self.roms.selected();
            let tag = profile_tag(entry);
            // The share of the game's achievements the player has, once the list studied it
            // (owner request: RetroAchievements on).
            let percent = achievement_percent(frame, &entry.path);
            let mut badges: Vec<(&str, [f32; 4])> = Vec::new();
            let colour = if selected && tag.1 == PROFILE {
                PROFILE_ON_FOCUS
            } else {
                tag.1
            };
            // A game of a page of games, downloaded when it is played (web build).
            if entry.remote.is_some() {
                badges.push(("[web]", DIM));
            }
            if entry.widescreen || entry.widescreen_other.is_some() {
                // Grey while off, coloured once on, red for a version of the game the hack is
                // not made for (owner request).
                // Orange once forced and on; dark grey while the ROM was not studied.
                let badge = if entry.widescreen_forced && entry.widescreen_on {
                    OTHER_MODE
                } else if entry.widescreen_unstudied {
                    UNSTUDIED
                } else if entry.widescreen_other.is_some() {
                    WARNING
                } else if entry.widescreen_on {
                    GAME_OWN
                } else {
                    DIM
                };
                badges.push(("[W]", badge));
            }
            if let Some(percent) = &percent {
                badges.push((percent, PERCENT));
            }
            let label = home_row_label(entry);
            if badges.is_empty() {
                painter.row(slot, selected, &label, (&tag.0, colour));
            } else {
                painter.row_badged(slot, selected, &label, &badges, (&tag.0, colour));
            }
        }
        // "Game options" in cyan when the selected game has some (owner request).
        let own = self
            .roms
            .selected_entry()
            .is_some_and(|entry| Self::has_game_options(frame.host, &entry.path));
        let mut hints: Vec<(&str, String, [f32; 4])> = layout::hints(Screen::Home)
            .iter()
            .map(|hint| {
                let colour = if own && hint.meaning == keys::HANDHELD_HINT_GAME_OPTIONS {
                    GAME_OWN
                } else {
                    TEXT
                };
                (hint.button, strings::get(hint.meaning), colour)
            })
            .collect();
        // L1 the widescreen of the selected game, in the colour of its [W]; R1 its hacks.
        if let Some(entry) = self
            .roms
            .selected_entry()
            .filter(|entry| entry.widescreen || entry.widescreen_other.is_some())
        {
            let colour = if entry.widescreen_forced && entry.widescreen_on {
                OTHER_MODE
            } else if entry.widescreen_unstudied {
                TEXT
            } else if entry.widescreen_other.is_some() {
                WARNING
            } else if entry.widescreen_on {
                GAME_OWN
            } else {
                TEXT
            };
            hints.push(("L1", strings::get(keys::HACK_WIDESCREEN), colour));
            hints.push(("R1", strings::get(keys::MENU_AIDS), TEXT));
        }
        // Xnes3D+: R2 opens the game's information (in HOME_HINTS), which has its achievements.
        // Xnes3D+: with no game listed, A picks the folder (Android).
        if total == 0 && crate::android_bridge::available() {
            hints = vec![
                ("A", "Choose ROM folder".to_owned(), TEXT),
                ("X", strings::get(keys::HANDHELD_HINT_OPTIONS), TEXT),
            ];
        }
        painter.hint_entries(&hints);
    }

    /// Xnes3D+: how wide the cover panel of the game list is: a third of a landscape screen,
    /// none on a narrow or portrait one.
    fn shelf_width(&self, size: [f32; 2]) -> f32 {
        if size[0] < size[1] * 1.2 || size[0] < self.metrics.font * 30.0 {
            return 0.0;
        }
        (size[0] * 0.3).round()
    }

    /// Xnes3D+: the cover panel: the box art of the selected game (or its name on a dark box
    /// when it has none yet), then its year, publisher, players and genre.
    fn draw_cover_panel(&self, painter: &Painter<'_>, shelf: f32) {
        let metrics = &self.metrics;
        let left = metrics.padding;
        let right = shelf - metrics.padding * 0.5;
        let top = metrics.title_band() + metrics.padding;
        let bottom = painter.size[1] - metrics.hint_band() - metrics.padding * 0.5;
        let width = right - left;
        let lines_room = metrics.font * 3.0 + metrics.padding;
        let art_room = (bottom - top - lines_room).max(metrics.row * 2.0);
        let entry = self.roms.selected_entry();
        let media = self.media();
        let cover = media.and_then(|media| media.cover);
        // An NES box is about 0.73 as wide as it is tall.
        let (box_width, box_height) = cover.map_or_else(
            || fit_size(73.0, 100.0, width, art_room),
            |(w, h)| fit_size(f32::from(u16::try_from(w).unwrap_or(1)), f32::from(u16::try_from(h).unwrap_or(1)), width, art_room),
        );
        let x = left + (width - box_width) * 0.5;
        let y = top;
        painter
            .list
            .add_rect([x + 5.0, y + 5.0], [x + box_width + 5.0, y + box_height + 5.0], [0.0, 0.0, 0.0, 1.0])
            .filled(true)
            .build();
        if cover.is_some() {
            painter
                .list
                .add_image(
                    imgui::TextureId::new(COVER_PICTURE_ID),
                    [x, y],
                    [x + box_width, y + box_height],
                )
                .build();
        } else {
            painter
                .list
                .add_rect([x, y], [x + box_width, y + box_height], [0.05, 0.05, 0.06, 1.0])
                .filled(true)
                .build();
            // A row of red dashes along the top, as on a box of the time.
            let dash = (metrics.padding * 0.8).max(6.0);
            let mut dash_x = x + metrics.padding;
            while dash_x + dash <= x + box_width - metrics.padding {
                painter
                    .list
                    .add_rect([dash_x, y + metrics.padding], [dash_x + dash, y + metrics.padding * 1.4], NES_RED)
                    .filled(true)
                    .build();
                dash_x += dash * 1.75;
            }
            let name = entry.map_or_else(String::new, |entry| entry.name.clone());
            let wrap = box_width - metrics.padding * 2.0;
            let height = painter.ui.calc_text_size_with_opts(&name, false, wrap)[1];
            painter.ui.set_cursor_pos([x + metrics.padding, y + (box_height - height) * 0.5]);
            let _wrap = painter.ui.push_text_wrap_pos_with_pos(x + metrics.padding + wrap);
            let _colour = painter.ui.push_style_color(StyleColor::Text, TEXT);
            painter.ui.text_wrapped(&name);
        }
        painter
            .list
            .add_rect([x, y], [x + box_width, y + box_height], [0.23, 0.23, 0.24, 1.0])
            .thickness(3.0)
            .build();
        // Under the box: what the information says, else a hint to get it.
        let summary = media
            .and_then(|media| media.info.as_ref())
            .map(crate::game_media::GameInfo::summary)
            .filter(|summary| !summary.is_empty());
        let (line, colour) = match summary {
            Some(summary) => (summary, DIM),
            None if entry.is_some_and(|entry| entry.resume) => (String::new(), DIM),
            None => (
                "No box art yet: Options > Games and 3D profiles > Download from LaunchBox"
                    .to_owned(),
                GROUP_TITLE,
            ),
        };
        if !line.is_empty() {
            painter.ui.set_cursor_pos([left, y + box_height + metrics.padding]);
            let _wrap = painter.ui.push_text_wrap_pos_with_pos(right);
            let _colour = painter.ui.push_style_color(StyleColor::Text, colour);
            painter.ui.text_wrapped(&line);
        }
    }

    /// Xnes3D+: the Game info screen: the picture of the game, its box and cartridge on the
    /// left; its name, details and description on the right.
    fn draw_game_info(&self, ui: &Ui, _frame: &UiFrame<'_>) {
        let Some(_window) = full_screen(ui, "##game-info", BACKGROUND) else {
            return;
        };
        let painter = self.painter(ui);
        let metrics = &self.metrics;
        painter.title("Game info", "LaunchBox Games Database");
        let Some(entry) = self.roms.selected_entry() else {
            painter.hints(layout::hints(Screen::GameInfo));
            return;
        };
        let media = self.media();
        let info = media.and_then(|media| media.info.clone()).unwrap_or_default();
        let size = painter.size;
        let top = metrics.title_band() + metrics.padding;
        let bottom = size[1] - metrics.hint_band() - metrics.padding;
        let portrait = size[0] < size[1];
        let column = if portrait { size[0] - metrics.padding * 2.0 } else { (size[0] * 0.38).round() };
        let left = metrics.padding;
        // The pictures: the game on top, the box and the cartridge side by side under it.
        let frame_box = |x: f32, y: f32, w: f32, h: f32, id: usize, picture: Option<(u32, u32)>, label: &str| {
            painter
                .list
                .add_rect([x, y], [x + w, y + h], [0.05, 0.05, 0.06, 1.0])
                .filled(true)
                .build();
            match picture {
                Some((pw, ph)) => {
                    let (iw, ih) = fit_size(
                        f32::from(u16::try_from(pw).unwrap_or(1)),
                        f32::from(u16::try_from(ph).unwrap_or(1)),
                        w - 6.0,
                        h - 6.0,
                    );
                    let ix = x + (w - iw) * 0.5;
                    let iy = y + (h - ih) * 0.5;
                    painter
                        .list
                        .add_image(imgui::TextureId::new(id), [ix, iy], [ix + iw, iy + ih])
                        .build();
                }
                None => {
                    let lw = painter.width(label);
                    painter.text(x + (w - lw) * 0.5, y + (h - metrics.font) * 0.5, GROUP_TITLE, label);
                }
            }
            painter
                .list
                .add_rect([x, y], [x + w, y + h], [0.23, 0.23, 0.24, 1.0])
                .thickness(3.0)
                .build();
        };
        let pictures_height = if portrait { (bottom - top) * 0.45 } else { bottom - top };
        let shot_height = (pictures_height * 0.58).min(column * 0.75);
        frame_box(left, top, column, shot_height, SHOT_PICTURE_ID, media.and_then(|m| m.shot), "No picture");
        let small_top = top + shot_height + metrics.padding;
        let small_height = (pictures_height - shot_height - metrics.padding).max(metrics.row);
        let half = (column - metrics.padding) * 0.5;
        frame_box(left, small_top, half, small_height, COVER_PICTURE_ID, media.and_then(|m| m.cover), "No box art");
        frame_box(left + half + metrics.padding, small_top, half, small_height, CART_PICTURE_ID, media.and_then(|m| m.cart), "No cartridge");
        // The text.
        let (text_left, text_top) = if portrait {
            (left, top + pictures_height + metrics.padding)
        } else {
            (left + column + metrics.padding * 1.5, top)
        };
        let text_right = size[0] - metrics.padding;
        let title = if info.title.is_empty() { entry.name.clone() } else { info.title.clone() };
        {
            let _heading = painter.heading.map(|font| ui.push_font(font));
            ui.set_cursor_pos([text_left, text_top]);
            let _wrap = ui.push_text_wrap_pos_with_pos(text_right);
            let _colour = ui.push_style_color(StyleColor::Text, TEXT);
            ui.text_wrapped(&title);
        }
        let mut details: Vec<(&str, String)> = Vec::new();
        for (label, value) in [
            ("Released", &info.year),
            ("Players", &info.players),
            ("Developer", &info.developer),
            ("Publisher", &info.publisher),
            ("Genre", &info.genre),
        ] {
            if !value.is_empty() {
                details.push((label, value.clone()));
            }
        }
        let y = ui.cursor_pos()[1] + metrics.padding * 0.5;
        ui.set_cursor_pos([text_left, y]);
        let _wrap = ui.push_text_wrap_pos_with_pos(text_right);
        for (label, value) in &details {
            ui.set_cursor_pos([text_left, ui.cursor_pos()[1]]);
            let _dim = ui.push_style_color(StyleColor::Text, DIM);
            ui.text(format!("{label}:"));
            ui.same_line();
            let _text = ui.push_style_color(StyleColor::Text, TEXT);
            ui.text_wrapped(value);
        }
        let y = ui.cursor_pos()[1] + metrics.padding * 0.5;
        painter
            .list
            .add_rect([text_left, y], [text_right, y + 3.0], NES_RED)
            .filled(true)
            .build();
        ui.set_cursor_pos([text_left, y + metrics.padding * 0.8]);
        let overview = if info.overview.is_empty() {
            if media.and_then(|media| media.info.as_ref()).is_some() {
                "No description for this game.".to_owned()
            } else {
                "No information yet. Options > Games and 3D profiles > Download from LaunchBox \
                 fetches it for every game of your list."
                    .to_owned()
            }
        } else {
            info.overview.clone()
        };
        // The description is cut where the hints start.
        let overview_top = ui.cursor_pos()[1];
        let _colour = ui.push_style_color(StyleColor::Text, DIM);
        painter.list.with_clip_rect_intersect([text_left, overview_top], [text_right, bottom], || {
            ui.text_wrapped(&overview);
        });
        painter.hints(layout::hints(Screen::GameInfo));
    }

    fn draw_game_options(&self, ui: &Ui, frame: &UiFrame<'_>) {
        let (Some(_window), Some(options)) = (
            full_screen(ui, "##game-options", BACKGROUND),
            &self.game_options,
        ) else {
            return;
        };
        let painter = self.painter(ui);
        let title = strings::format(
            &strings::get(keys::HANDHELD_GAME_OPTIONS_TITLE),
            &[options.name()],
        );
        let selected = options.selected_index();
        painter.title(&title, &page_path(&options.path()));
        // Q-103, read once for the screen, as the console's global pages do.
        let run_ahead_forced = frame
            .host
            .session()
            .is_some_and(crate::game_session::GameSession::forces_run_ahead_to_zero);
        // RO-3: the game's pages are the same tree as the global ones, reduced to what a game can
        // set. Every row of a page is an option row; the main screen has three that are not.
        let main: Vec<GameRow> = options.page_rows().map_or_else(
            || GAME_MAIN.to_vec(),
            |page| page.iter().copied().map(GameRow::Option).collect(),
        );
        let mut specs = Vec::with_capacity(main.len());
        for (index, row) in main.iter().enumerate() {
            let focused = index == selected;
            let own_colour = |own: bool| match (own, focused) {
                (true, true) => GAME_OWN_ON_FOCUS,
                (true, false) => GAME_OWN,
                (false, _) => DIM,
            };
            let plain = |left: String, right: String, colour: [f32; 4]| RowSpec::Row {
                left,
                badges: Vec::new(),
                right,
                colour,
            };
            specs.push(match row {
                GameRow::ResetAll => plain(
                    strings::get(keys::HANDHELD_GAME_OPTIONS_RESET),
                    String::new(),
                    own_colour(options.has_options()),
                ),
                GameRow::Widescreen | GameRow::Hacks => {
                    let (label, value) = zaxnes_row_text(*row, options.edited());
                    plain(label, value, DIM)
                }
                GameRow::Option(options::OptionRow::Group(key)) => {
                    RowSpec::Group(strings::get(key))
                }
                // RO-3 §2 rule 2: a row that climbs to a page says how many values of its own the
                // game has under it, and nothing when it has none.
                GameRow::Option(options::OptionRow::Page(page)) => {
                    let count = options.own_count_text(*page);
                    let shown = if count.is_empty() {
                        ">".to_owned()
                    } else {
                        format!("{count} >")
                    };
                    plain(
                        options::page_title(*page),
                        shown,
                        own_colour(!count.is_empty()),
                    )
                }
                GameRow::Option(row) => {
                    let own = options.is_own(*row);
                    let shown = format!("< {} >", options.console_value_text(*row));
                    let shown = match options::forced_run_ahead_note(*row, run_ahead_forced) {
                        Some(note) => format!("{shown} {note}"),
                        None => shown,
                    };
                    let shown = match options::engine_lock_note(*row, options.edited()) {
                        Some(note) => format!("{shown} {note}"),
                        None => shown,
                    };
                    // RO-3 §2 rule 1: the global value is dimmed, the game's own is marked.
                    let badges = if own {
                        vec![(GAME_OWN_MARKER, GAME_OWN)]
                    } else {
                        Vec::new()
                    };
                    RowSpec::Row {
                        left: options::label(*row),
                        badges,
                        right: shown,
                        colour: own_colour(own),
                    }
                }
            });
        }
        painter.spec_list(&specs, selected, painter.list_room());
        painter.hints(layout::hints(Screen::GameOptions));
    }

    /// The hacks of one game: the widescreen, its ratio and margin, the options of its patch.
    fn draw_hacks(&self, ui: &Ui) {
        let (Some(_window), Some(hacks)) = (full_screen(ui, "##hacks", BACKGROUND), &self.hacks)
        else {
            return;
        };
        let painter = self.painter(ui);
        let title = strings::format(&strings::get(keys::HACKS_TITLE), &[hacks.name()]);
        painter.title(&title, "");
        // The detail of how the hack fits this ROM, in place of the rows.
        if hacks.compat_open() {
            let lines: Vec<_> = hacks
                .compat_lines()
                .into_iter()
                .map(|(text, tone)| {
                    let colour = match tone {
                        Tone::Good => PROFILE,
                        Tone::Warning => OTHER_MODE,
                        Tone::Bad => WARNING,
                        Tone::Plain => DIM,
                    };
                    (text, colour)
                })
                .collect();
            painter.lines(&lines);
            painter.hints(layout::hints(Screen::Hacks));
            return;
        }
        let rows = hacks.rows();
        if rows.is_empty() {
            match hacks.other_rom_message() {
                Some(message) => painter.message(&message, WARNING),
                None => painter.message(&strings::get(keys::HACKS_NONE), DIM),
            }
        }
        // Another version of the game: what it is, under the rows that force the patch.
        let other = hacks.other_rom_message().filter(|_| !rows.is_empty());
        // The note on the patch takes the place of a row or two at the bottom.
        let visible = self.metrics.visible_rows(painter.size[1]);
        let visible = if other.is_some() {
            visible.saturating_sub(3).max(1)
        } else if hacks.patch().is_some() {
            visible.saturating_sub(2).max(1)
        } else {
            visible
        };
        let selected = hacks.selected_index();
        let specs: Vec<RowSpec> = rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let colour = match (hacks.is_own(*row), index == selected) {
                    (true, true) => GAME_OWN_ON_FOCUS,
                    (true, false) => GAME_OWN,
                    (false, _) => DIM,
                };
                RowSpec::Row {
                    left: hacks.label(*row),
                    badges: Vec::new(),
                    right: format!("< {} >", hacks.value_text(*row)),
                    colour,
                }
            })
            .collect();
        #[allow(clippy::cast_precision_loss)] // A few rows.
        let room = (visible as f32 * self.metrics.row).min(painter.list_room());
        painter.spec_list(&specs, selected, room);
        if let Some(other) = other {
            painter.note(visible, &other);
        } else if let Some(note) = hacks.patch_note() {
            painter.note(visible, &strings::get(note));
        }
        painter.hints(layout::hints(Screen::Hacks));
    }

    fn draw_profiles(&self, ui: &Ui) {
        let (Some(_window), Some(profiles)) =
            (full_screen(ui, "##profiles", BACKGROUND), &self.profiles)
        else {
            return;
        };
        let painter = self.painter(ui);
        let title = strings::format(
            &strings::get(keys::HANDHELD_PROFILE_FOR),
            &[profiles.name()],
        );
        painter.title(&title, "");
        let rows = self.metrics.visible_rows(painter.size[1]);
        let total = profiles.choices().len();
        let first = painter.window(profiles.selected(), rows, total);
        for (slot, (index, choice)) in profiles
            .choices()
            .iter()
            .enumerate()
            .skip(first)
            .take(rows)
            .enumerate()
        {
            painter.row(
                slot,
                index == profiles.selected(),
                &choice.title,
                (&choice.source_label(), DIM),
            );
        }
        painter.hints(layout::hints(Screen::Profiles));
    }

    fn draw_options(&self, ui: &Ui, frame: &UiFrame<'_>) {
        let Some(_window) = full_screen(ui, "##options", BACKGROUND) else {
            return;
        };
        let painter = self.painter(ui);
        // RO-3: the pages are three deep, so the title band spells the path out rather than the
        // innermost page's name alone.
        let page = page_path(&self.options.path());
        painter.title(&text::options_title(), &page);
        // The list scrolls with the selection when it is longer than the screen.
        let selected = self.options.selected_index();
        let list = self.options.rows();
        // Q-103: whether the game being played holds Run-ahead at 0, read once for the page.
        let run_ahead_forced = frame
            .host
            .session()
            .is_some_and(crate::game_session::GameSession::forces_run_ahead_to_zero);
        let mut specs = Vec::with_capacity(list.len());
        for row in list {
            if let options::OptionRow::Group(_) = row {
                specs.push(RowSpec::Group(options::label(*row)));
                continue;
            }
            let value = options::value_text(*row, frame.host.settings(), self.overlay.camera());
            let shown = match row {
                options::OptionRow::Controls => String::new(),
                options::OptionRow::Page(_) => ">".to_owned(),
                options::OptionRow::Overscan | options::OptionRow::Achievements => {
                    format!("{value} >")
                }
                // Xnes3D+: opened with A, nothing to step.
                options::OptionRow::Android(_) => format!("{value} >"),
                _ => format!("< {value} >"),
            };
            // The setting is kept, so the row still steps; what it says is that this game runs
            // at 0 all the same.
            let shown = match options::forced_run_ahead_note(*row, run_ahead_forced) {
                Some(note) => format!("{shown} {note}"),
                None => shown,
            };
            // A control only Zaxnes's own engine has, while another engine is chosen.
            let shown = match options::engine_lock_note(*row, frame.host.settings()) {
                Some(note) => format!("{shown} {note}"),
                None => shown,
            };
            // Xnes3D+: a row that holds something other than its default says so.
            let badges = if options::is_changed(*row, frame.host.settings()) == Some(true) {
                vec![(CHANGED_MARKER, WARNING)]
            } else {
                Vec::new()
            };
            specs.push(RowSpec::Row {
                left: options::row_label(*row, frame.host.settings()),
                badges,
                right: shown,
                colour: DIM,
            });
        }
        // Xnes3D+: what the selected row does, in a box under the list.
        let help = super::plain::help(self.options.selected());
        let help_height = help.map_or(0.0, |help| painter.help_box(help));
        painter.spec_list(&specs, selected, painter.list_room() - help_height);
        painter.hints(layout::hints(Screen::Options));
    }

    /// The overscan (Rust-only): the NTSC and PAL sides and the mode, or a game's own.
    fn draw_overscan(&self, ui: &Ui, frame: &UiFrame<'_>) {
        let Some(_window) = full_screen(ui, "##overscan", BACKGROUND) else {
            return;
        };
        let painter = self.painter(ui);
        let game = self
            .game_options
            .as_ref()
            .filter(|_| self.overscan.is_game());
        let title = strings::get(keys::HANDHELD_OPTION_OVERSCAN);
        let title = game.map_or(title.clone(), |options| {
            format!("{title} : {}", options.name())
        });
        painter.title(&title, "");
        let ids = self.overscan.rows();
        let selected = self.overscan.selected_index();
        let mut specs = Vec::with_capacity(ids.len());
        for (index, id) in ids.iter().enumerate() {
            let focused = index == selected;
            let (shown, colour) = match game {
                Some(options) => {
                    let own = options.edited().has_game_value(*id);
                    let value = overscan::value_text(*id, options.edited());
                    let colour = match (own, focused) {
                        (true, true) => GAME_OWN_ON_FOCUS,
                        (true, false) => GAME_OWN,
                        (false, _) => DIM,
                    };
                    let value = if own || *id != SettingId::VideoOverscanMode {
                        value
                    } else {
                        strings::format(&strings::get(keys::HANDHELD_USE_GLOBAL), &[&value])
                    };
                    (value, colour)
                }
                None => (overscan::value_text(*id, frame.host.settings()), DIM),
            };
            specs.push(RowSpec::Row {
                left: overscan::label(*id),
                badges: Vec::new(),
                right: format!("< {shown} >"),
                colour,
            });
        }
        painter.spec_list(&specs, selected, painter.list_room());
        painter.hints(layout::hints(Screen::Overscan));
    }

    fn draw_controls(&mut self, ui: &Ui, frame: &UiFrame<'_>) {
        let Some(_window) = full_screen(ui, "##controls", BACKGROUND) else {
            return;
        };
        let rows = self.control_rows();
        let painter = self.painter(ui);
        painter.title(&strings::get(keys::HANDHELD_CONTROLS_PLAYER), "");
        let session = frame.input.rebinding();
        let set = key_config::set_of(
            frame.input.bindings(),
            frame.input.pads(),
            0,
            BindingDeviceKind::Gamepad,
        );
        if set.is_none() {
            painter.message(&text::no_gamepad(), WARNING);
            painter.hints(layout::hints(Screen::Controls));
            return;
        }
        // The list scrolls with the selection when it is longer than the screen.
        let visible = self.metrics.visible_rows(painter.size[1]);
        let first = painter.window(self.controls.selected, visible, rows.len());
        for (slot, (index, row)) in rows
            .iter()
            .enumerate()
            .skip(first)
            .take(visible)
            .enumerate()
        {
            let listening = key_config::is_listening_for(session, row, BindingDeviceKind::Gamepad);
            let binding = if listening {
                text::press_a_control()
            } else {
                set.map_or_else(String::new, |set| labels::of(set.get(&row.target)))
            };
            painter.row(
                slot,
                index == self.controls.selected,
                &row.label,
                (&binding, DIM),
            );
        }
        let hints = if self.controls.capture_left.is_some() {
            layout::CAPTURE_HINTS
        } else {
            layout::hints(Screen::Controls)
        };
        painter.hints(hints);
    }

    fn draw_overlay(&self, ui: &Ui, frame: &UiFrame<'_>) {
        let size = ui.io().display_size;
        ui.get_background_draw_list()
            .add_rect([0.0, 0.0], size, [0.0, 0.0, 0.0, 0.6])
            .filled(true)
            .build();
        let Some(_window) = full_screen(ui, "##overlay", [0.0, 0.0, 0.0, 0.0]) else {
            return;
        };
        let painter = self.painter(ui);
        let metrics = &self.metrics;
        // The menu is a panel in the middle, so the game stays visible around it.
        let panel_width = (size[0] - 4.0 * metrics.padding).min(metrics.font * 24.0);
        let left = (size[0] - panel_width) * 0.5;
        let heights: Vec<f32> = self
            .overlay
            .items()
            .iter()
            .map(|item| {
                let value = self.overlay_value(*item, frame);
                painter.row_height(Self::overlay_wraps(
                    &painter,
                    *item,
                    &value,
                    left,
                    panel_width,
                ))
            })
            .collect();
        // Taller than the screen (owner request): the rows scroll with the selection, as the
        // lists do, an arrow saying more rows are above or below.
        let room = (size[1] - metrics.hint_band() - metrics.title_band() - metrics.padding * 2.0)
            .max(metrics.row);
        let (first, last) = overlay_window(
            &heights,
            self.overlay.selected_index(),
            self.overlay_first.get(),
            room,
        );
        self.overlay_first.set(first);
        let rows: f32 = heights[first..last].iter().sum();
        let panel_height = metrics.title_band() + rows + metrics.padding;
        let top = ((size[1] - metrics.hint_band() - panel_height) * 0.5).max(0.0);
        // Xnes3D+: square corners, a grey title band with the red stripe and a grey frame.
        painter
            .list
            .add_rect([left + 6.0, top + 6.0], [left + panel_width + 6.0, top + panel_height + 6.0], [0.0, 0.0, 0.0, 1.0])
            .filled(true)
            .build();
        painter
            .list
            .add_rect([left, top], [left + panel_width, top + panel_height], PANEL)
            .filled(true)
            .build();
        let band = metrics.title_band();
        painter
            .list
            .add_rect([left, top], [left + panel_width, top + band], TITLE_BAND)
            .filled(true)
            .build();
        painter
            .list
            .add_rect([left, top + band - (metrics.padding * 0.3).max(2.0)], [left + panel_width, top + band], NES_RED)
            .filled(true)
            .build();
        painter
            .list
            .add_rect([left, top], [left + panel_width, top + panel_height], TITLE_BAND)
            .thickness(3.0)
            .build();
        self.draw_overlay_rows(&painter, frame, (left, top, panel_width), (first, last));
        // Triangles drawn, not glyphs: the interface's face may lack them.
        let half = (metrics.font * 0.3).round();
        let x = left + panel_width - metrics.padding * 1.5 - half;
        let y = top + metrics.title_band() * 0.5;
        if first > 0 {
            painter
                .list
                .add_triangle(
                    [x - half, y - half * 0.3],
                    [x + half, y - half * 0.3],
                    [x, y - half * 1.5],
                    TITLE_DIM,
                )
                .filled(true)
                .build();
        }
        if last < heights.len() {
            painter
                .list
                .add_triangle(
                    [x - half, y + half * 0.3],
                    [x + half, y + half * 0.3],
                    [x, y + half * 1.5],
                    TITLE_DIM,
                )
                .filled(true)
                .build();
        }
        painter.hints(layout::hints(Screen::Overlay));
    }

    fn draw_overlay_rows(
        &self,
        painter: &Painter<'_>,
        frame: &UiFrame<'_>,
        (left, top, width): (f32, f32, f32),
        (first, last): (usize, usize),
    ) {
        let metrics = &self.metrics;
        {
            let _heading = painter.heading.map(|font| painter.ui.push_font(font));
            let title = if self.overlay.in_more() {
                "More".to_owned()
            } else {
                strings::get(keys::HANDHELD_MENU_TITLE)
            };
            painter.text(left + metrics.padding, top + metrics.padding, TITLE_TEXT, &title);
        }
        let mut row_top = top + metrics.title_band();
        for (index, item) in self
            .overlay
            .items()
            .iter()
            .enumerate()
            .take(last)
            .skip(first)
        {
            let selected = index == self.overlay.selected_index();
            let value = self.overlay_value(*item, frame);
            // Owner request: a value that does not fit beside its label goes under it.
            let wrapped = Self::overlay_wraps(painter, *item, &value, left, width);
            let height = painter.row_height(wrapped);
            if selected {
                painter
                    .list
                    .add_rect(
                        [left + metrics.padding * 0.5, row_top],
                        [left + width - metrics.padding * 0.5, row_top + height],
                        FOCUS,
                    )
                    .filled(true)
                    .build();
                // Xnes3D+: the red edge of the focus bar.
                painter
                    .list
                    .add_rect(
                        [left + metrics.padding * 0.5, row_top],
                        [left + metrics.padding * 0.9, row_top + height],
                        NES_RED,
                    )
                    .filled(true)
                    .build();
            }
            let y = row_top + (metrics.row - metrics.font) * 0.5;
            let (value_x, value_y) = if wrapped {
                (left + metrics.padding * 2.5, y + painter.wrap_step())
            } else {
                (
                    left + width - metrics.padding * 1.5 - painter.width(&value),
                    y,
                )
            };
            let value_end = if wrapped {
                value_x + painter.width(&value) + metrics.padding
            } else {
                left + width
            };
            painter.zone(
                [left + metrics.padding * 0.5, row_top],
                [left + width - metrics.padding * 0.5, row_top + height],
                ZoneAction::Row {
                    rank: index,
                    selected,
                    value: (!value.is_empty()).then_some((value_x, value_end)),
                },
            );
            let colour = if selected { ON_FOCUS } else { TEXT };
            painter.text(
                left + metrics.padding * 1.5,
                y,
                colour,
                &Self::overlay_label(*item),
            );
            if !value.is_empty() {
                painter.text(
                    value_x,
                    value_y,
                    if selected { ON_FOCUS } else { DIM },
                    &value,
                );
            }
            row_top += height;
        }
    }

    /// Whether the in-game menu's row `item` with `value` overflows the panel `width` wide at
    /// `left`: its value then goes under its label.
    fn overlay_wraps(
        painter: &Painter<'_>,
        item: OverlayItem,
        value: &str,
        left: f32,
        width: f32,
    ) -> bool {
        let padding = painter.metrics.padding;
        painter.wraps(
            &Self::overlay_label(item),
            &[],
            value,
            (left + padding, left + width - padding * 0.5),
        )
    }

    fn overlay_label(item: OverlayItem) -> String {
        strings::get(match item {
            OverlayItem::Resume => keys::HANDHELD_MENU_RESUME,
            OverlayItem::SaveState => keys::HANDHELD_MENU_SAVE_STATE,
            OverlayItem::LoadState => keys::HANDHELD_MENU_LOAD_STATE,
            OverlayItem::Speed => keys::HANDHELD_MENU_GAME_SPEED,
            OverlayItem::Camera => keys::HANDHELD_OPTION_CAMERA,
            OverlayItem::Options => keys::WINDOW_OPTIONS,
            OverlayItem::Widescreen => keys::HACK_WIDESCREEN,
            OverlayItem::Hacks => keys::MENU_AIDS,
            OverlayItem::Achievements => keys::TAB_ACHIEVEMENTS,
            OverlayItem::QuitToList => keys::HANDHELD_MENU_QUIT_TO_LIST,
            OverlayItem::WebOpenRom => keys::HANDHELD_MENU_WEB_OPEN_ROM,
            OverlayItem::WebImportProfile => keys::HANDHELD_MENU_WEB_IMPORT_PROFILE,
            OverlayItem::WebManageCache => keys::HANDHELD_MENU_WEB_MANAGE_CACHE,
            OverlayItem::ExitToLauncher => keys::HANDHELD_MENU_EXIT,
            // Xnes3D+: plain English, as the fork's other texts.
            OverlayItem::More => "More",
        })
    }

    fn overlay_value(&self, item: OverlayItem, frame: &UiFrame<'_>) -> String {
        match item {
            OverlayItem::SaveState | OverlayItem::LoadState => {
                let slot = self.overlay.slot();
                let record = frame.host.slots.find(&numbered_slot_id(slot));
                format!("<{}>", menu_bar::slot_short_label(slot, record))
            }
            OverlayItem::Camera => format!("< {} >", options::camera_label(self.overlay.camera())),
            OverlayItem::Speed => format!(
                "< {} >",
                crate::emulation::speed_setting::factor_text(
                    f64::from(self.overlay.speed()) / 10.0
                )
            ),
            OverlayItem::More => ">".to_owned(),
            OverlayItem::Widescreen | OverlayItem::Hacks if !playing_with_hacks(frame.host) => {
                strings::get(keys::HANDHELD_ZAXNES_ONLY)
            }
            OverlayItem::Widescreen => {
                let settings = frame.host.settings();
                let on = settings.get(SettingId::VideoWidescreen).as_int() != 0;
                format!(
                    "{} - {}",
                    strings::get(if on { keys::HACK_ON } else { keys::HACK_OFF }),
                    strings::get(keys::HANDHELD_HOLD_FOR_SETTINGS)
                )
            }
            _ => String::new(),
        }
    }

    /// The save state screen: the slots on the left, the thumbnail of the selected one on the
    /// right, in the colour of what A does (save or load), which the title says too.
    fn draw_states(&self, ui: &Ui, frame: &UiFrame<'_>) {
        let Some(_window) = full_screen(ui, "##states", BACKGROUND) else {
            return;
        };
        let painter = self.painter(ui);
        let metrics = &self.metrics;
        let states = self.states();
        let (accent, title) = match states.mode() {
            StatesMode::Save => (SAVE_ACCENT, keys::HANDHELD_STATES_SAVE_TITLE),
            StatesMode::Load => (LOAD_ACCENT, keys::HANDHELD_STATES_LOAD_TITLE),
        };
        let size = painter.size;
        let band = metrics.title_band();
        painter
            .list
            .add_rect([0.0, 0.0], [size[0], band], TITLE_BAND)
            .filled(true)
            .build();
        painter
            .list
            .add_rect([0.0, band - metrics.padding * 0.3], [size[0], band], accent)
            .filled(true)
            .build();
        {
            let _heading = painter.heading.map(|font| ui.push_font(font));
            painter.text(
                metrics.padding,
                metrics.padding,
                TITLE_TEXT,
                &strings::get(title),
            );
        }

        // The slots, on the left.
        let split = (size[0] * 0.56).round();
        let total = crate::persistence::save_slots::CONSOLE_SLOT_COUNT;
        let rows = metrics.visible_rows(size[1]);
        let first = painter.window(states.selected_index(), rows, total);
        for (row, index) in (first..total).take(rows).enumerate() {
            let slot = i32::try_from(index + 1).unwrap_or(1);
            let record = frame.host.slots.find(&numbered_slot_id(slot));
            let colour = match slot_state(frame.host, slot) {
                SlotState::Empty => DIM,
                SlotState::Fits => TEXT,
                SlotState::OtherMode => OTHER_MODE,
                SlotState::OtherVersion => WARNING,
            };
            let label = menu_bar::slot_label(slot, record);
            let (name, date) = label.split_once(" — ").unwrap_or((label.as_str(), ""));
            // The engine that made the state, when it is not Zaxnes or the game runs on another;
            // red when it is not the engine in play, whose state it cannot load.
            let (date, colour) =
                match menu_bar::slot_engine_note(frame.host, &numbered_slot_id(slot)) {
                    Some(menu_bar::EngineNote::Shown(note)) => (format!("{date} · {note}"), colour),
                    Some(menu_bar::EngineNote::Refused(_)) => {
                        let engine = frame.host.slot_engine(&numbered_slot_id(slot));
                        let name = engine.map_or("?", zax_core_api::EngineKind::display_name);
                        (format!("{date} · {name}"), WARNING)
                    }
                    None => (date.to_owned(), colour),
                };
            painter.row_to(
                row,
                (index == states.selected_index(), accent),
                name,
                (&date, colour),
                split - metrics.padding * 0.5,
            );
        }

        self.draw_state_preview(&painter, frame, accent, split);
        painter.hints(layout::states_hints(states.mode()));
    }

    /// The thumbnail of the selected slot, framed in `accent`, right of `split`; under it, why
    /// the state does not load as it is, when it does not.
    fn draw_state_preview(
        &self,
        painter: &Painter<'_>,
        frame: &UiFrame<'_>,
        accent: [f32; 4],
        split: f32,
    ) {
        let ui = painter.ui;
        let metrics = &self.metrics;
        let size = painter.size;
        let states = self.states();
        let left = split + metrics.padding * 0.5;
        let right = size[0] - metrics.padding;
        let top = painter.row_top(0);
        let bottom = size[1] - metrics.hint_band() - metrics.padding;
        let width = (right - left).max(1.0);
        let height = (width * 0.75).min((bottom - top - 2.0 * metrics.row).max(1.0));
        let width = height / 0.75;
        let x = left + ((right - left) - width) * 0.5;
        painter
            .list
            .add_rect(
                [x - 3.0, top - 3.0],
                [x + width + 3.0, top + height + 3.0],
                accent,
            )
            .rounding(metrics.padding * 0.3)
            .thickness(3.0)
            .build();
        let slot = states.slot();
        if self.state_picture_shown() {
            painter
                .list
                .add_image(
                    imgui::TextureId::new(STATE_PICTURE_ID),
                    [x, top],
                    [x + width, top + height],
                )
                .build();
        } else {
            let empty = !frame.host.slots.is_occupied(&numbered_slot_id(slot));
            let message = if empty {
                text::empty_slot()
            } else {
                strings::get(keys::HANDHELD_STATES_NO_PICTURE)
            };
            let message_x = x + (width - painter.width(&message)) * 0.5;
            painter.text(
                message_x,
                top + (height - metrics.font) * 0.5,
                DIM,
                &message,
            );
        }
        // Under it, why the state does not load as it is, when it does not.
        let caption = match slot_state(frame.host, slot) {
            SlotState::OtherMode => Some((keys::SLOT_OTHER_PATCH, OTHER_MODE)),
            SlotState::OtherVersion => Some((keys::SLOT_OTHER_HACK_VERSION, WARNING)),
            SlotState::Empty | SlotState::Fits => None,
        };
        let caption = caption.map(|(key, colour)| (strings::get(key), colour));
        // A state of another engine than the game's says so first: that is what stops it.
        let caption = match menu_bar::slot_engine_note(frame.host, &numbered_slot_id(slot)) {
            Some(menu_bar::EngineNote::Refused(note)) => Some((note, WARNING)),
            Some(menu_bar::EngineNote::Shown(note)) if caption.is_none() => Some((note, DIM)),
            _ => caption,
        };
        if let Some((message, colour)) = caption {
            ui.set_cursor_pos([left, top + height + metrics.padding]);
            let _wrap = ui.push_text_wrap_pos_with_pos(right);
            let _colour = ui.push_style_color(StyleColor::Text, colour);
            ui.text_wrapped(message);
        }
    }

    /// A question of the host, over the screen: its title and text, A to agree, B to decline.
    fn draw_confirmation(&self, ui: &Ui, question: ConfirmationView<'_>) {
        let size = ui.io().display_size;
        let metrics = &self.metrics;
        let Some(_window) = full_screen_over(ui, "##question", [0.0, 0.0, 0.0, 0.75]) else {
            return;
        };
        // Only its buttons answer the pointer, not the screen under it.
        self.zones.borrow_mut().clear();
        let painter = self.painter(ui);
        let width = (size[0] - 4.0 * metrics.padding).min(metrics.font * 26.0);
        let left = (size[0] - width) * 0.5;
        let top = size[1] * 0.2;
        let bottom = size[1] - metrics.hint_band() - 2.0 * metrics.padding;
        painter
            .list
            .add_rect([left, top], [left + width, bottom], PANEL)
            .filled(true)
            .rounding(metrics.padding)
            .build();
        {
            let _heading = painter.heading.map(|font| ui.push_font(font));
            painter.text(
                left + metrics.padding,
                top + metrics.padding,
                FOCUS,
                question.title(),
            );
        }
        ui.set_cursor_pos([left + metrics.padding, top + metrics.title_band()]);
        let _wrap = ui.push_text_wrap_pos_with_pos(left + width - metrics.padding);
        let _colour = ui.push_style_color(StyleColor::Text, TEXT);
        ui.text_wrapped(question.message());
        // A message to acknowledge has no cancelling button.
        let cancel = question.cancel_label();
        if cancel.is_empty() {
            painter.hint_texts(&[("A", question.confirm_label())]);
        } else if let Some(alternative) = question.alternative_label() {
            painter.hint_texts(&[
                ("A", question.confirm_label()),
                ("X", alternative),
                ("B", cancel),
            ]);
        } else {
            painter.hint_texts(&[("A", question.confirm_label()), ("B", cancel)]);
        }
    }

    /// While Select + Start is held: how long until the application quits.
    fn draw_exit_hold(&self, ui: &Ui) {
        let progress = self.combo.progress();
        if progress <= 0.0 {
            return;
        }
        let list = ui.get_foreground_draw_list();
        let size = ui.io().display_size;
        let metrics = &self.metrics;
        let bottom = size[1] - metrics.hint_band() - metrics.padding;
        let top = bottom - metrics.row;
        list.add_rect([0.0, top], [size[0], bottom], [0.0, 0.0, 0.0, 0.85])
            .filled(true)
            .build();
        list.add_rect([0.0, top], [size[0] * progress, bottom], WARNING)
            .filled(true)
            .build();
        let message = strings::get(keys::HANDHELD_EXIT_HOLD);
        let x = (size[0] - ui.calc_text_size(&message)[0]) * 0.5;
        list.add_text([x, top + (metrics.row - metrics.font) * 0.5], TEXT, message);
    }
}

/// The name a row of the home list shows. AMD-053 REQ-UI-210: a favourite entry is distinguished;
/// the form is free, and this one is a star before the name. REQ-LOOP-076: the Resume row says
/// what it is, with the title of its game.
fn home_row_label(entry: &RomEntry) -> String {
    if entry.resume {
        format!(
            "{} — {}",
            strings::get(&Command::ResumeLast.label_key()),
            entry.name
        )
    } else if entry.favorite {
        format!("{FAVORITE_MARKER} {}", entry.name)
    } else {
        entry.name.clone()
    }
}

/// The tag at the right of a game: its 3D profile, or "2D" when it has none.
///
/// Xnes3D+: a game with no profile plays in Smart 3D, and says so.
fn profile_tag(entry: &RomEntry) -> (String, [f32; 4]) {
    match &entry.profile {
        Some(profile) if !profile.two_dimensional => (profile.title.clone(), PROFILE),
        Some(_) => (strings::get(keys::HANDHELD_TAG_2D), DIM),
        None => ("Smart 3D".to_owned(), DIM),
    }
}

/// RetroAchievements in the console mode (Rust-only, owner request).
impl HandheldUi {
    /// The switches and the account.
    fn draw_achievements(&self, ui: &Ui, frame: &UiFrame<'_>) {
        let Some(_window) = full_screen(ui, "##achievements", BACKGROUND) else {
            return;
        };
        let painter = self.painter(ui);
        let system = &frame.host.achievements;
        painter.title(
            &strings::get(keys::TAB_ACHIEVEMENTS),
            &crate::ui::achievements_view::account_text(system),
        );
        let signed_in = matches!(
            system.account(),
            crate::achievements::Account::SignedIn { .. }
        );
        let settings = frame.host.settings();
        let rows = self.metrics.visible_rows(painter.size[1]).saturating_sub(1);
        let selected = self.achievements.selected_index();
        let all = super::achievements::ROWS;
        let first = painter.window(selected, rows, all.len());
        for (slot, (index, row)) in all.iter().enumerate().skip(first).take(rows).enumerate() {
            let switch = |id| {
                let on = settings.get(id).as_bool();
                format!(
                    "< {} >",
                    text::label_of_option(if on { "enabled" } else { "disabled" })
                )
            };
            let (label, value) = match row {
                AchievementsRow::Username => (
                    strings::get(keys::ACH_USERNAME),
                    format!("{} >", self.achievements.username),
                ),
                AchievementsRow::Password => (
                    strings::get(keys::ACH_PASSWORD),
                    format!(
                        "{} >",
                        "\u{2022}".repeat(self.achievements.password.chars().count())
                    ),
                ),
                AchievementsRow::Account if signed_in => {
                    (strings::get(keys::ACH_SIGN_OUT), String::new())
                }
                AchievementsRow::Account => (strings::get(keys::ACH_SIGN_IN), String::new()),
                AchievementsRow::List => (
                    strings::get(keys::ACH_LIST),
                    system.game().map_or_else(String::new, |game| {
                        format!("{}/{} >", game.summary.unlocked, game.summary.core)
                    }),
                ),
                switch_row => (
                    switch_row
                        .setting()
                        .map(|id| text::label_of(zax_settings::catalogue::descriptor(id)))
                        .unwrap_or_default(),
                    switch_row.setting().map(switch).unwrap_or_default(),
                ),
            };
            painter.row(slot, index == selected, &label, (&value, DIM));
        }
        let note = if settings.get(SettingId::AchievementsEnabled).as_bool() {
            strings::get(keys::ACH_SOFTCORE_HINT)
        } else {
            strings::get(keys::ACH_TURN_ON_HINT)
        };
        painter.note(rows.min(all.len()), &note);
        painter.hints(layout::hints(Screen::Achievements));
    }

    /// The keyboard on the screen: the text typed, then the rows of keys and the bottom row.
    fn draw_keyboard(&self, ui: &Ui) {
        let Some(_window) = full_screen(ui, "##keyboard", BACKGROUND) else {
            return;
        };
        let Some(keyboard) = self.keyboard.as_ref() else {
            return;
        };
        let painter = self.painter(ui);
        let metrics = self.metrics;
        let title = match keyboard.field() {
            super::achievements::Field::Username => strings::get(keys::ACH_USERNAME),
            super::achievements::Field::Password => strings::get(keys::ACH_PASSWORD),
        };
        painter.title(&title, "");
        let width = painter.size[0];
        let field_top = metrics.title_band() + metrics.padding;
        painter
            .list
            .add_rect(
                [metrics.padding, field_top],
                [width - metrics.padding, field_top + metrics.row],
                BAR,
            )
            .filled(true)
            .rounding(metrics.padding * 0.4)
            .build();
        let shown = format!("{}_", keyboard.shown_text());
        painter.text(
            metrics.padding * 2.0,
            field_top + (metrics.row - metrics.font) * 0.5,
            TEXT,
            &shown,
        );
        let keys_top = field_top + metrics.row + metrics.padding;
        let room = painter.size[1] - metrics.hint_band() - metrics.padding - keys_top;
        let key_height = (room / 5.0).min(metrics.row);
        let (cursor_row, cursor_column) = keyboard.cursor();
        let gap = metrics.padding * 0.3;
        let draw_key = |left: f32, top: f32, key_width: f32, label: &str, selected: bool| {
            let colour = if selected { FOCUS } else { PANEL };
            painter
                .list
                .add_rect(
                    [left + gap, top + gap],
                    [left + key_width - gap, top + key_height - gap],
                    colour,
                )
                .filled(true)
                .rounding(metrics.padding * 0.3)
                .build();
            let text_colour = if selected { ON_FOCUS } else { TEXT };
            painter.text(
                left + (key_width - painter.width(label)) * 0.5,
                top + (key_height - metrics.font) * 0.5,
                text_colour,
                label,
            );
        };
        let usable = width - 2.0 * metrics.padding;
        for (row, characters) in keyboard.rows().iter().enumerate() {
            let count = characters.chars().count();
            #[allow(clippy::cast_precision_loss)] // at most ten keys
            let key_width = usable / 10.0;
            #[allow(clippy::cast_precision_loss)] // at most ten keys
            let left = metrics.padding + (usable - key_width * count as f32) * 0.5;
            #[allow(clippy::cast_precision_loss)] // five rows
            let top = keys_top + key_height * row as f32;
            for (column, character) in characters.chars().enumerate() {
                #[allow(clippy::cast_precision_loss)] // at most ten keys
                let x = left + key_width * column as f32;
                let selected = (row, column) == (cursor_row, cursor_column);
                draw_key(x, top, key_width, &character.to_string(), selected);
            }
        }
        let shift = ["abc", "ABC", "#+="][(keyboard.layout() + 1) % 3];
        let top = keys_top + key_height * 4.0;
        #[allow(clippy::cast_precision_loss)] // four keys
        let key_width = usable / SPECIAL_KEYS.len() as f32;
        for (column, key) in SPECIAL_KEYS.iter().enumerate() {
            let label = match key {
                SpecialKey::Shift => shift.to_owned(),
                SpecialKey::Space => strings::get(keys::ACH_KEY_SPACE),
                SpecialKey::Delete => strings::get(keys::ACH_KEY_DELETE),
                SpecialKey::Done => strings::get(keys::ACH_KEY_DONE),
            };
            #[allow(clippy::cast_precision_loss)] // four keys
            let x = metrics.padding + key_width * column as f32;
            let selected = (4, column) == (cursor_row, cursor_column);
            draw_key(x, top, key_width, &label, selected);
        }
        painter.hints(layout::hints(Screen::Keyboard));
    }

    /// The game's achievements, scrolled with the gamepad.
    /// A game's achievements as a grid of icons (owner request): the unlocked first, the
    /// others grey; the selected one told under the grid.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )] // small positive counts and sizes
    fn draw_achievement_grid(&self, ui: &Ui, frame: &UiFrame<'_>) {
        let (Some(_window), Some((grid, _))) = (
            full_screen(ui, "##achievement_grid", BACKGROUND),
            &self.grid,
        ) else {
            return;
        };
        let painter = self.painter(ui);
        let metrics = self.metrics;
        let system = &frame.host.achievements;
        let Some(game) = super::achievement_grid::game_of(grid.source(), system) else {
            painter.title(&strings::get(keys::ACH_LIST), "");
            let studying = match grid.source() {
                super::achievement_grid::GridSource::Rom(path) => system.is_studying(path),
                super::achievement_grid::GridSource::Playing => false,
            };
            let message = if studying {
                strings::get(keys::ACH_GRID_STUDYING)
            } else if !matches!(
                system.account(),
                crate::achievements::Account::SignedIn { .. }
            ) {
                strings::get(keys::ACH_GRID_SIGNED_OUT)
            } else {
                strings::get(keys::ACH_GRID_NONE)
            };
            painter.message(&message, DIM);
            painter.hints(layout::hints(Screen::AchievementGrid));
            return;
        };
        let total = game.achievements.len();
        let unlocked = game.unlocked();
        let count = format!(
            "{unlocked}/{total}  {}",
            crate::ui::achievements_view::percent_text(
                u32::try_from(unlocked).unwrap_or(0),
                u32::try_from(total).unwrap_or(1)
            )
        );
        painter.title(&game.title, &count);
        let top = metrics.title_band() + metrics.padding * 0.5;
        let info = metrics.row * 2.0;
        let bottom = painter.size[1] - metrics.hint_band() - info;
        let icon = metrics.row * 1.4;
        let gap = metrics.padding * 0.6;
        let cell = icon + gap;
        let width = painter.size[0] - 2.0 * metrics.padding;
        let columns = (((width + gap) / cell).floor() as usize).max(1);
        grid.laid_out(columns, total);
        let lines = (((bottom - top + gap) / cell).floor() as usize).max(1);
        let selected = grid.selected();
        let line = selected / columns;
        let first_line = (line + 1).saturating_sub(lines);
        let x0 = (painter.size[0] - (columns as f32 * cell - gap)) * 0.5;
        for (index, achievement) in game.achievements.iter().enumerate() {
            let line = index / columns;
            if line < first_line || line >= first_line + lines {
                continue;
            }
            let x = x0 + (index % columns) as f32 * cell;
            let y = top + (line - first_line) as f32 * cell;
            if index == selected {
                let pad = gap * 0.45;
                painter
                    .list
                    .add_rect([x - pad, y - pad], [x + icon + pad, y + icon + pad], NES_RED)
                    .filled(true)
                    .rounding(pad)
                    .build();
            }
            let url = if achievement.unlocked {
                &achievement.badge_url
            } else {
                &achievement.badge_locked_url
            };
            match system.badge(url) {
                Some(id) => painter
                    .list
                    .add_image(imgui::TextureId::new(id), [x, y], [x + icon, y + icon])
                    .build(),
                None => painter
                    .list
                    .add_rect([x, y], [x + icon, y + icon], BAR)
                    .filled(true)
                    .build(),
            }
        }
        if let Some(achievement) = game.achievements.get(selected) {
            draw_grid_choice(&painter, achievement, bottom + metrics.padding * 0.3);
        }
        painter.hints(layout::hints(Screen::AchievementGrid));
    }

    fn draw_achievement_list(&self, ui: &Ui, frame: &UiFrame<'_>) {
        let Some(_window) = full_screen(ui, "##achievement_list", BACKGROUND) else {
            return;
        };
        let painter = self.painter(ui);
        let metrics = self.metrics;
        painter.title(&strings::get(keys::ACH_LIST), "");
        let top = metrics.title_band() + metrics.padding * 0.5;
        let height = painter.size[1] - metrics.hint_band() - top;
        ui.set_cursor_pos([metrics.padding, top]);
        ui.child_window("##achievement_list_items")
            .size([painter.size[0] - 2.0 * metrics.padding, height])
            .scroll_bar(false)
            .build(|| {
                ui.set_scroll_y(self.list_scroll);
                crate::ui::achievements_view::draw_list(ui, &frame.host.achievements);
                self.list_scroll_max.set(ui.scroll_max_y());
            });
        painter.hints(layout::hints(Screen::AchievementList));
    }
}

/// Whether the engine of the game in play carries the widescreen extension and the hack packs
/// (or no game is in play): Zaxnes, and NesHawk since lot NESHAWK-HACKS; FCEUX has not got them.
pub(super) fn playing_with_hacks(host: &crate::application::AppHost) -> bool {
    host.session().is_none_or(|session| {
        zax_core_api::Capabilities::of_kind(session.core().kind())
            .has(zax_core_api::Capability::Widescreen)
    })
}

/// The label and value of the widescreen and hacks rows of a game's main options screen: what
/// they open, or that they are Zaxnes's own while the game runs on another engine.
fn zaxnes_row_text(row: GameRow, model: &zax_settings::model::SettingsModel) -> (String, String) {
    let label = strings::get(if row == GameRow::Widescreen {
        keys::HACK_WIDESCREEN
    } else {
        keys::MENU_AIDS
    });
    let capability = if row == GameRow::Widescreen {
        zax_core_api::Capability::Widescreen
    } else {
        zax_core_api::Capability::HackPacks
    };
    let value = if !crate::ui::options::engine_has(model, capability) {
        strings::get(keys::HANDHELD_ZAXNES_ONLY)
    } else if row == GameRow::Widescreen {
        strings::get(keys::HANDHELD_HOLD_FOR_SETTINGS)
    } else {
        ">".to_owned()
    };
    (label, value)
}

/// The rows `first..last` of a menu whose rows are `heights` tall that fit in `room`: from
/// `previous` (the first row shown before), moved only as far as the `selected` row needs; all
/// of them when they fit.
fn overlay_window(heights: &[f32], selected: usize, previous: usize, room: f32) -> (usize, usize) {
    let total = heights.len();
    if total == 0 || heights.iter().sum::<f32>() <= room {
        return (0, total);
    }
    let selected = selected.min(total - 1);
    // The end of the rows from `first` that fit (one at least).
    let fit = |first: usize| {
        let mut used = 0.0;
        let mut last = first;
        while last < total && (last == first || used + heights[last] <= room) {
            used += heights[last];
            last += 1;
        }
        last
    };
    let mut first = previous.min(selected);
    while fit(first) <= selected {
        first += 1;
    }
    // No empty room at the bottom while rows wait above.
    while first > 0 && fit(first - 1) >= total {
        first -= 1;
    }
    (first, fit(first))
}

#[cfg(test)]
mod overlay_window_tests {
    use super::overlay_window;

    #[test]
    fn the_menu_scrolls_only_when_it_does_not_fit() {
        let rows = [1.0; 10];
        assert_eq!((0, 10), overlay_window(&rows, 9, 0, 10.0));
        assert_eq!((0, 4), overlay_window(&rows, 0, 0, 4.0));
        assert_eq!((0, 4), overlay_window(&rows, 3, 0, 4.0));
        assert_eq!((1, 5), overlay_window(&rows, 4, 0, 4.0));
        // Going back up keeps the window until the selection leaves it.
        assert_eq!((1, 5), overlay_window(&rows, 2, 1, 4.0));
        assert_eq!((0, 4), overlay_window(&rows, 0, 1, 4.0));
        // Wrapping round to the last row shows the end.
        assert_eq!((6, 10), overlay_window(&rows, 9, 0, 4.0));
    }
}

/// Xnes3D+: the largest size of a `width` x `height` picture that fits `room_width` x
/// `room_height`, keeping its shape.
fn fit_size(width: f32, height: f32, room_width: f32, room_height: f32) -> (f32, f32) {
    if width <= 0.0 || height <= 0.0 {
        return (room_width, room_height);
    }
    let scale = (room_width / width).min(room_height / height);
    ((width * scale).round(), (height * scale).round())
}
