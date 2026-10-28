//! Layout decisions use GTK logical pixels; resizing never reloads a service.
pub const MIN_WIDTH: i32 = 320;
pub const MIN_HEIGHT: i32 = 240;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Sidebar,
    BottomBar,
}

impl Mode {
    pub fn for_size(width: i32, height: i32) -> Self {
        if width < 640 || height < 460 {
            Self::BottomBar
        } else {
            Self::Sidebar
        }
    }
}

pub fn initial_size(work_width: i32, work_height: i32, preferred: (i32, i32)) -> (i32, i32) {
    if work_width <= 0 || work_height <= 0 {
        return preferred;
    }
    (
        preferred
            .0
            .min(work_width.saturating_sub(32))
            .max(MIN_WIDTH),
        preferred
            .1
            .min(work_height.saturating_sub(32))
            .max(MIN_HEIGHT),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_or_short_windows_use_a_bottom_bar() {
        for size in [(320, 240), (420, 640), (639, 740), (1100, 459)] {
            assert_eq!(Mode::for_size(size.0, size.1), Mode::BottomBar);
        }
        for size in [(640, 460), (640, 480), (1100, 740)] {
            assert_eq!(Mode::for_size(size.0, size.1), Mode::Sidebar);
        }
    }

    #[test]
    fn initial_windows_and_sign_in_popups_fit_the_workarea() {
        assert_eq!(initial_size(1920, 1080, (1100, 740)), (1100, 740));
        assert_eq!(initial_size(800, 600, (1100, 740)), (768, 568));
        assert_eq!(initial_size(800, 600, (780, 680)), (768, 568));
        assert_eq!(initial_size(360, 320, (1100, 740)), (328, 288));
        assert_eq!(initial_size(0, 0, (1100, 740)), (1100, 740));
    }
}
