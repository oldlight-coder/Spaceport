//! Where the video goes inside the main window: all of it, or a small corner box (the mini
//! player) while the user keeps browsing.

/// A rectangle in the main window's client area, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// The mini player's width as a share of the window, within these limits (physical pixels).
const MINI_SHARE: f64 = 0.3;
const MINI_MIN_WIDTH: u32 = 320;
const MINI_MAX_WIDTH: u32 = 640;

/// The video's place in a client area of `width` x `height`. The mini player is 16:9, in the
/// bottom-right corner with a margin, and never larger than the window allows.
pub fn video_rect(width: u32, height: u32, mini: bool) -> Rect {
    if !mini {
        return Rect {
            x: 0,
            y: 0,
            width,
            height,
        };
    }
    let margin = (width / 60).max(16);
    let room_w = width.saturating_sub(2 * margin).max(1);
    let room_h = height.saturating_sub(2 * margin).max(1);
    let wanted = ((f64::from(width) * MINI_SHARE) as u32).clamp(MINI_MIN_WIDTH, MINI_MAX_WIDTH);
    let mut w = wanted.min(room_w);
    let mut h = w * 9 / 16;
    if h > room_h {
        h = room_h;
        w = h * 16 / 9;
    }
    Rect {
        x: (width - margin - w) as i32,
        y: (height.saturating_sub(margin + h)) as i32,
        width: w,
        height: h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_size_fills_the_window() {
        assert_eq!(
            video_rect(1920, 1080, false),
            Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080
            }
        );
    }

    #[test]
    fn mini_sits_in_the_bottom_right_corner() {
        let r = video_rect(1920, 1080, true);
        assert_eq!((r.width, r.height), (576, 324));
        assert_eq!(r.x + r.width as i32, 1920 - 32);
        assert_eq!(r.y + r.height as i32, 1080 - 32);
    }

    #[test]
    fn mini_stays_inside_small_windows() {
        let r = video_rect(800, 300, true);
        assert!(r.x >= 0 && r.y >= 0);
        assert!(r.x as u32 + r.width <= 800 && r.y as u32 + r.height <= 300);
        assert_eq!(r.width * 9 / 16, r.height);
        // A 30% share of a 1200 px window is 360 px; narrower windows get the minimum.
        assert_eq!(video_rect(1200, 900, true).width, 360);
        assert_eq!(video_rect(900, 900, true).width, MINI_MIN_WIDTH);
    }
}
