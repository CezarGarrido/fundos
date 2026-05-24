use egui::Color32;

pub struct ThemeColors;

impl ThemeColors {
    // Backgrounds
    pub fn bg(dark: bool) -> Color32 {
        if dark {
            Color32::from_rgb(16, 20, 30)
        } else {
            Color32::from_rgb(248, 250, 253)
        }
    }

    pub fn panel_bg(dark: bool) -> Color32 {
        if dark {
            Color32::from_rgb(22, 26, 34)
        } else {
            Color32::from_rgb(245, 247, 251)
        }
    }

    pub fn card_bg(dark: bool) -> Color32 {
        if dark {
            Color32::from_rgb(30, 35, 45) // Slightly lighter than panel_bg
        } else {
            Color32::from_rgb(255, 255, 255)
        }
    }

    // Text Colors
    pub fn text_primary(dark: bool) -> Color32 {
        if dark {
            Color32::from_rgb(250, 250, 250) // Crisp white for dark mode
        } else {
            Color32::from_rgb(10, 10, 10) // Crisp black for light mode
        }
    }

    pub fn text_muted(dark: bool) -> Color32 {
        if dark {
            Color32::from_rgb(200, 200, 200) // Clear readable light gray
        } else {
            Color32::from_rgb(40, 50, 70) // Dark readable slate
        }
    }

    pub fn header(dark: bool) -> Color32 {
        if dark {
            Color32::from_rgb(255, 255, 255)
        } else {
            Color32::from_rgb(0, 0, 0)
        }
    }

    // Semantic / Branding Colors
    pub fn accent() -> Color32 {
        Color32::from_rgb(37, 99, 235) // Deep Blue
    }

    pub fn success() -> Color32 {
        Color32::from_rgb(34, 197, 94) // Green
    }

    pub fn danger() -> Color32 {
        Color32::from_rgb(239, 68, 68) // Red
    }

    pub fn warning() -> Color32 {
        Color32::from_rgb(234, 179, 8) // Yellow
    }

    pub fn info() -> Color32 {
        Color32::from_rgb(59, 130, 246) // Lighter blue
    }
}
