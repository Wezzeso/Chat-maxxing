use serde::Deserialize;

/// Document dimensions in CSS pixels, measured inside the service page.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Geometry {
    pub viewport: f64,
    pub client: f64,
    pub content: f64,
}

impl Geometry {
    pub fn valid(&self) -> bool {
        [self.viewport, self.client, self.content]
            .into_iter()
            .all(|value| value.is_finite() && value > 0.0 && value <= 65_536.0)
            && self.client <= self.viewport + 2.0
            && self.content >= self.client - 2.0
    }

    pub fn overflows(&self) -> bool {
        self.content > self.client + 4.0
    }
}

/// Retain an observed minimum document width for the current page. A fitting
/// measurement at reduced zoom must not discard it and trigger a zoom loop.
#[derive(Default, Debug)]
pub struct Fit {
    pub required_width: f64,
    pub scrollbar: f64,
}

impl Fit {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn zoom(&self, native_width: i32) -> f64 {
        if self.required_width <= 0.0 || native_width <= 0 {
            return 1.0;
        }
        let available = (native_width as f64 - self.scrollbar).max(1.0);
        let zoom = (available / self.required_width).min(1.0);
        ((zoom * 1_000.0).floor() / 1_000.0).clamp(0.4, 1.0)
    }

    /// Return whether this observation establishes document overflow. In CSS
    /// pixels content width is independent of page zoom; scrollbar width is
    /// converted back to native pixels before calculating the next zoom.
    pub fn observe(&mut self, geometry: Geometry, at_zoom: f64) -> bool {
        if !geometry.valid() || !at_zoom.is_finite() || at_zoom <= 0.0 {
            return false;
        }
        self.scrollbar =
            ((geometry.viewport - geometry.client).max(0.0) * at_zoom).clamp(0.0, 64.0);
        if !geometry.overflows() {
            return false;
        }
        self.required_width = self.required_width.max(geometry.content);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry(viewport: f64, client: f64, content: f64) -> Geometry {
        Geometry {
            viewport,
            client,
            content,
        }
    }

    #[test]
    fn responsive_pages_keep_full_size() {
        let mut fit = Fit::default();
        assert!(!fit.observe(geometry(420.0, 420.0, 420.0), 1.0));
        assert_eq!(fit.zoom(420), 1.0);
        assert!(!fit.observe(geometry(320.0, 320.0, 324.0), 1.0));
        assert_eq!(fit.zoom(320), 1.0);
    }

    #[test]
    fn observed_fixed_width_fits_portrait_and_returns_to_full_size_when_wide() {
        let mut fit = Fit::default();
        assert!(fit.observe(geometry(420.0, 420.0, 748.0), 1.0));
        assert_eq!(fit.zoom(420), 0.561);
        assert_eq!(fit.zoom(320), 0.427);
        assert_eq!(fit.zoom(1044), 1.0);
    }

    #[test]
    fn fitting_zoomed_measurements_keep_the_required_width_stable() {
        let mut fit = Fit::default();
        assert!(fit.observe(geometry(420.0, 420.0, 748.0), 1.0));
        let zoom = fit.zoom(420);
        for _ in 0..20 {
            assert!(!fit.observe(geometry(420.0 / zoom, 420.0 / zoom, 420.0 / zoom), zoom));
            assert_eq!(fit.required_width, 748.0);
            assert_eq!(fit.zoom(420), zoom);
        }
    }

    #[test]
    fn invalid_geometry_and_zoom_leave_cached_values_unchanged() {
        let mut fit = Fit::default();
        assert!(fit.observe(geometry(420.0, 410.0, 748.0), 1.0));
        for sample in [
            geometry(f64::NAN, 410.0, 748.0),
            geometry(420.0, f64::INFINITY, 748.0),
            geometry(420.0, 410.0, f64::NAN),
            geometry(-420.0, 410.0, 748.0),
            geometry(420.0, 0.0, 748.0),
            geometry(420.0, 423.0, 748.0),
            geometry(420.0, 410.0, 407.0),
            geometry(65_537.0, 410.0, 748.0),
            geometry(420.0, 410.0, 65_537.0),
        ] {
            assert!(!fit.observe(sample, 1.0), "{sample:?}");
            assert_eq!(fit.required_width, 748.0);
            assert_eq!(fit.scrollbar, 10.0);
        }
        for zoom in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            assert!(!fit.observe(geometry(320.0, 320.0, 1_000.0), zoom));
            assert_eq!(fit.required_width, 748.0);
            assert_eq!(fit.scrollbar, 10.0);
        }
    }

    #[test]
    fn minimum_zoom_and_nonpositive_native_width_are_bounded() {
        let mut fit = Fit::default();
        assert!(fit.observe(geometry(320.0, 320.0, 65_536.0), 1.0));
        assert_eq!(fit.zoom(320), 0.4);
        assert_eq!(fit.zoom(1), 0.4);
        assert_eq!(fit.zoom(0), 1.0);
        assert_eq!(fit.zoom(-1), 1.0);
    }

    #[test]
    fn scrollbar_uses_native_pixels_and_updates_even_when_content_fits() {
        let mut fit = Fit::default();
        assert!(fit.observe(geometry(420.0, 405.0, 748.0), 1.0));
        assert_eq!(fit.scrollbar, 15.0);
        assert_eq!(fit.zoom(420), 0.541);
        assert!(!fit.observe(geometry(800.0, 770.0, 770.0), 0.5));
        assert_eq!(fit.scrollbar, 15.0);
        assert_eq!(fit.required_width, 748.0);
        assert!(!fit.observe(geometry(800.0, 801.0, 801.0), 0.5));
        assert_eq!(fit.scrollbar, 0.0);
        assert!(fit.observe(geometry(420.0, 320.0, 748.0), 1.0));
        assert_eq!(fit.scrollbar, 64.0);
    }

    #[test]
    fn observations_only_increase_required_width_until_reset() {
        let mut fit = Fit::default();
        assert!(fit.observe(geometry(420.0, 420.0, 748.0), 1.0));
        assert!(fit.observe(geometry(420.0, 420.0, 600.0), 1.0));
        assert_eq!(fit.required_width, 748.0);
        assert!(fit.observe(geometry(420.0, 420.0, 900.0), 1.0));
        assert_eq!(fit.required_width, 900.0);
        fit.reset();
        assert_eq!(fit.required_width, 0.0);
        assert_eq!(fit.scrollbar, 0.0);
        assert_eq!(fit.zoom(320), 1.0);
    }
}
