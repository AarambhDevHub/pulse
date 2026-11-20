//! Start-up splash: a rat walks in from the right, stands up, shakes its
//! behind for three dance steps, then walks off to the left. After that the
//! real application starts.
//!
//! The animation is a pure function of elapsed time ([`scene`]), so it is easy
//! to test. [`draw`] renders one frame and [`run`] plays the whole thing.

use std::time::{Duration, Instant};

use color_eyre::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::prelude::*;

use crate::theme::Theme;
use crate::ui::{MIN_HEIGHT, MIN_WIDTH};

/// Width and height of every art frame, in terminal cells.
pub const ART_WIDTH: usize = 19;
pub const ART_HEIGHT: usize = 9;

// ---- timeline ------------------------------------------------------------

const ENTER: Duration = Duration::from_millis(1700);
const RISE: Duration = Duration::from_millis(350);
/// One hip swing (left *or* right).
const SWING: Duration = Duration::from_millis(300);
/// A dance step is a swing left plus a swing right.
pub const DANCE_STEPS: u32 = 3;
const DROP: Duration = Duration::from_millis(350);
const EXIT: Duration = Duration::from_millis(1400);
/// How long a walking leg frame is shown.
const STRIDE: Duration = Duration::from_millis(140);
/// Redraw rate while the splash plays.
const FRAME: Duration = Duration::from_millis(40);

fn dance_len() -> Duration {
    SWING * 2 * DANCE_STEPS
}

/// Total running time of the splash.
pub fn total() -> Duration {
    ENTER + RISE + dance_len() + DROP + EXIT
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
    /// Walking on four legs, left feet forward.
    WalkA,
    /// Walking on four legs, right feet forward.
    WalkB,
    /// Halfway between all fours and standing.
    Crouch,
    /// Standing, hips pushed to the left.
    HipLeft,
    /// Standing, hips pushed to the right.
    HipRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scene {
    pub pose: Pose,
    /// Column of the art's left edge. Negative or past the width while the
    /// rat is partly (or fully) off screen.
    pub x: i32,
    /// Which dance step (0-based) is playing; 0 outside the dance.
    pub step: u32,
}

/// Where the rat is, and what it is doing, `elapsed` after the start.
/// Returns `None` once the splash is over.
pub fn scene(elapsed: Duration, width: u16) -> Option<Scene> {
    let width = i32::from(width);
    let art = ART_WIDTH as i32;
    let center = (width - art) / 2;

    let walk = |t: Duration| {
        if (t.as_millis() / STRIDE.as_millis()).is_multiple_of(2) {
            Pose::WalkA
        } else {
            Pose::WalkB
        }
    };
    let lerp = |from: i32, to: i32, p: f64| from + (f64::from(to - from) * p).round() as i32;

    let mut t = elapsed;
    if t < ENTER {
        let p = t.as_secs_f64() / ENTER.as_secs_f64();
        let eased = 1.0 - (1.0 - p).powi(2); // slows down as it arrives
        return Some(Scene {
            pose: walk(t),
            x: lerp(width, center, eased),
            step: 0,
        });
    }
    t -= ENTER;

    if t < RISE {
        return Some(Scene {
            pose: Pose::Crouch,
            x: center,
            step: 0,
        });
    }
    t -= RISE;

    if t < dance_len() {
        let swing = (t.as_millis() / SWING.as_millis()) as u32;
        return Some(Scene {
            pose: if swing.is_multiple_of(2) {
                Pose::HipLeft
            } else {
                Pose::HipRight
            },
            x: center,
            step: swing / 2,
        });
    }
    t -= dance_len();

    if t < DROP {
        return Some(Scene {
            pose: Pose::Crouch,
            x: center,
            step: 0,
        });
    }
    t -= DROP;

    if t < EXIT {
        let p = t.as_secs_f64() / EXIT.as_secs_f64();
        let eased = p * p; // speeds up as it leaves
        return Some(Scene {
            pose: walk(t),
            x: lerp(center, -art, eased),
            step: 0,
        });
    }
    None
}

// ---- art -----------------------------------------------------------------

// Everything below is plain ASCII so one char is one cell.

const HEAD: [&str; 4] = ["  __   __  ", " (__)-(__) ", "  ( o.o )  ", " ==( Y )== "];

/// A standing, dancing rat. Only the hips, legs and tail move.
fn dancing(hip_shift: i32, arms_up: bool, tail: &str) -> [String; ART_HEIGHT] {
    let arms = if arms_up {
        r"\_/     \_/"
    } else {
        r"  /     \  "
    };
    let rows = [
        HEAD[0].to_owned(),
        HEAD[1].to_owned(),
        HEAD[2].to_owned(),
        HEAD[3].to_owned(),
        arms.to_owned(),
        " (       ) ".to_owned(),
        format!("(_________){tail}"),
        r"  /|   |\  ".to_owned(),
        "  d     b  ".to_owned(),
    ];
    let mut out: [String; ART_HEIGHT] = Default::default();
    for (i, row) in rows.iter().enumerate() {
        // Head, arms and belly stay put; hips, legs and feet swing.
        let shift = if i >= 6 { hip_shift } else { 0 };
        out[i] = place(row, 4 + shift);
    }
    out
}

/// Four-legged rat, facing left.
fn walking(legs: &str) -> [String; ART_HEIGHT] {
    let rows = [
        "",
        "",
        "",
        "",
        "  __ __",
        r" (__ __)___",
        r"=( o.o )    \__~~",
        r"  (Y)\_______/",
        legs,
    ];
    let mut out: [String; ART_HEIGHT] = Default::default();
    for (i, row) in rows.iter().enumerate() {
        out[i] = place(row, 1);
    }
    out
}

fn crouching() -> [String; ART_HEIGHT] {
    let rows = [
        "",
        "",
        "  __ __",
        " (__ __)",
        " =( o.o )",
        r"   (Y)  \",
        r"    (____ )~~",
        r"    /|  |\",
        "    d    b",
    ];
    let mut out: [String; ART_HEIGHT] = Default::default();
    for (i, row) in rows.iter().enumerate() {
        out[i] = place(row, 2);
    }
    out
}

/// Indent `row` by `indent` columns (can be negative) and clip to the frame.
fn place(row: &str, indent: i32) -> String {
    let mut s = if indent >= 0 {
        format!("{}{row}", " ".repeat(indent as usize))
    } else {
        row.to_owned()
    };
    s.truncate(ART_WIDTH);
    s
}

/// The art for a pose, one string per row.
pub fn art(pose: Pose) -> [String; ART_HEIGHT] {
    match pose {
        Pose::WalkA => walking(r"    /|      |\"),
        Pose::WalkB => walking(r"     |\    /|"),
        Pose::Crouch => crouching(),
        Pose::HipLeft => dancing(-2, true, "~~~"),
        Pose::HipRight => dancing(2, false, "~"),
    }
}

// ---- rendering -----------------------------------------------------------

/// Write `line` into the buffer starting at column `x`, clipping at the
/// edges. Spaces are transparent.
fn blit(buf: &mut Buffer, area: Rect, x: i32, y: u16, line: &str, style: Style) {
    if y < area.top() || y >= area.bottom() {
        return;
    }
    for (i, ch) in line.chars().enumerate() {
        if ch == ' ' {
            continue;
        }
        let col = x + i as i32;
        if col < i32::from(area.left()) || col >= i32::from(area.right()) {
            continue;
        }
        buf[(col as u16, y)].set_char(ch).set_style(style);
    }
}

fn centered(buf: &mut Buffer, area: Rect, y: u16, text: &str, style: Style) {
    let x = (i32::from(area.width) - text.chars().count() as i32) / 2;
    blit(buf, area, x, y, text, style);
}

/// Draw the splash as it looks `elapsed` after the start.
pub fn draw(frame: &mut Frame, elapsed: Duration, theme: &Theme) {
    let area = frame.area();
    let Some(scene) = scene(elapsed, area.width) else {
        return;
    };
    let buf = frame.buffer_mut();

    let top = (area.height.saturating_sub(ART_HEIGHT as u16) / 2).max(4);
    let ground = top + ART_HEIGHT as u16;
    let accent = Style::new().fg(theme.accent);
    let dim = Style::new().fg(theme.dim);

    centered(buf, area, top - 3, "P  U  L  S  E", accent.bold());
    if scene.pose == Pose::HipLeft || scene.pose == Pose::HipRight {
        centered(buf, area, top - 2, "terminal system monitor", dim);
    }

    // Floor.
    blit(
        buf,
        area,
        0,
        ground,
        &"─".repeat(usize::from(area.width)),
        dim,
    );

    // The rat.
    for (row, line) in art(scene.pose).iter().enumerate() {
        blit(buf, area, scene.x, top + row as u16, line, accent);
    }

    // Music notes while dancing.
    match scene.pose {
        Pose::HipLeft => blit(buf, area, scene.x + ART_WIDTH as i32, top + 1, "♪", dim),
        Pose::HipRight => blit(buf, area, scene.x - 2, top + 1, "♫", dim),
        _ => {}
    }

    centered(buf, area, area.height - 1, "press any key to skip", dim);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Played to the end, or the terminal was too small to bother.
    Done,
    /// The user pressed a key.
    Skipped,
    /// The user pressed Ctrl+C: leave the program.
    Quit,
}

/// Play the splash. It reads the keyboard itself, so call it before the
/// input thread starts.
pub fn run(terminal: &mut DefaultTerminal, theme: &Theme) -> Result<Outcome> {
    let size = terminal.size()?;
    if size.width < MIN_WIDTH || size.height < MIN_HEIGHT {
        return Ok(Outcome::Done);
    }

    let start = Instant::now();
    while start.elapsed() < total() {
        let elapsed = start.elapsed();
        terminal.draw(|frame| draw(frame, elapsed, theme))?;

        if event::poll(FRAME)?
            && let Event::Key(key) = event::read()?
            && key.kind != KeyEventKind::Release
        {
            let ctrl_c =
                key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c');
            return Ok(if ctrl_c {
                Outcome::Quit
            } else {
                Outcome::Skipped
            });
        }
    }
    Ok(Outcome::Done)
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    const ALL_POSES: [Pose; 5] = [
        Pose::WalkA,
        Pose::WalkB,
        Pose::Crouch,
        Pose::HipLeft,
        Pose::HipRight,
    ];

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn art_is_ascii_and_fits_the_frame() {
        for pose in ALL_POSES {
            for line in art(pose) {
                assert!(line.is_ascii(), "{pose:?}: {line:?}");
                assert!(line.len() <= ART_WIDTH, "{pose:?}: {line:?} is too wide");
            }
        }
    }

    #[test]
    fn rat_starts_off_screen_right_and_ends_off_screen_left() {
        let start = scene(Duration::ZERO, 100).unwrap();
        assert_eq!(start.x, 100);

        let almost_end = scene(total() - ms(1), 100).unwrap();
        assert!(
            almost_end.x <= -(ART_WIDTH as i32) + 1,
            "x = {}",
            almost_end.x
        );
    }

    #[test]
    fn splash_ends() {
        assert!(scene(total(), 100).is_none());
        assert!(scene(total() + ms(5000), 100).is_none());
    }

    #[test]
    fn rat_arrives_at_the_centre_and_stays_for_the_dance() {
        let center = (100 - ART_WIDTH as i32) / 2;
        assert_eq!(scene(ENTER, 100).unwrap().x, center);
        assert_eq!(scene(ENTER + RISE + ms(10), 100).unwrap().x, center);
        assert_eq!(
            scene(ENTER + RISE + dance_len() - ms(1), 100).unwrap().x,
            center
        );
    }

    #[test]
    fn rat_only_ever_moves_left() {
        let mut last = i32::MAX;
        let mut t = Duration::ZERO;
        while let Some(s) = scene(t, 120) {
            assert!(s.x <= last, "moved right at {t:?}: {} -> {}", last, s.x);
            last = s.x;
            t += ms(10);
        }
    }

    #[test]
    fn dance_is_three_steps_of_left_then_right() {
        let dance_start = ENTER + RISE;
        let mut poses = Vec::new();
        for i in 0..(DANCE_STEPS * 2) {
            let s = scene(dance_start + SWING * i + ms(5), 100).unwrap();
            poses.push((s.pose, s.step));
        }
        assert_eq!(
            poses,
            [
                (Pose::HipLeft, 0),
                (Pose::HipRight, 0),
                (Pose::HipLeft, 1),
                (Pose::HipRight, 1),
                (Pose::HipLeft, 2),
                (Pose::HipRight, 2),
            ]
        );
    }

    #[test]
    fn walking_alternates_legs() {
        let a = scene(ms(10), 100).unwrap().pose;
        let b = scene(ms(10) + STRIDE, 100).unwrap().pose;
        assert_ne!(a, b);
        assert!(matches!(a, Pose::WalkA | Pose::WalkB));
    }

    #[test]
    fn hips_actually_move_between_swings() {
        assert_ne!(art(Pose::HipLeft)[6], art(Pose::HipRight)[6]);
        // The head does not move.
        assert_eq!(art(Pose::HipLeft)[1], art(Pose::HipRight)[1]);
    }

    fn render(width: u16, height: u16, elapsed: Duration) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let theme = Theme::dark();
        terminal.draw(|f| draw(f, elapsed, &theme)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .chunks(usize::from(width))
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn every_moment_renders_without_panicking_at_several_sizes() {
        for (w, h) in [(80, 24), (100, 30), (200, 60), (30, 12), (1, 1)] {
            let mut t = Duration::ZERO;
            while t < total() + ms(100) {
                render(w, h, t);
                t += ms(97);
            }
        }
    }

    #[test]
    fn dance_frame_shows_title_rat_and_hint() {
        let text = render(100, 30, ENTER + RISE + ms(10));
        for expected in [
            "P  U  L  S  E",
            "terminal system monitor",
            "( o.o )",
            "press any key",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
        }
    }

    #[test]
    fn rat_is_not_visible_before_it_enters() {
        let text = render(100, 30, Duration::ZERO);
        assert!(!text.contains("o.o"), "{text}");
    }

    /// Prints every pose so the art can be checked by eye:
    /// `cargo test preview_art -- --ignored --nocapture`
    #[test]
    #[ignore = "prints art for eyeballing"]
    fn preview_art() {
        for pose in ALL_POSES {
            println!("--- {pose:?} ---");
            for line in art(pose) {
                println!("|{line:<w$}|", w = ART_WIDTH);
            }
        }
    }
}
