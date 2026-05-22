use super::colors::ThemeColors;
use egui::RichText;

pub struct Typography;

impl Typography {
    pub fn heading_1(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(26.0)
            .strong()
            .color(ThemeColors::header(dark))
    }

    pub fn heading_2(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(20.0)
            .strong()
            .color(ThemeColors::header(dark))
    }

    pub fn heading_3(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(16.0)
            .strong()
            .color(ThemeColors::header(dark))
    }

    pub fn body(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(14.0)
            .color(ThemeColors::text_primary(dark))
    }

    pub fn body_strong(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(14.0)
            .strong()
            .color(ThemeColors::text_primary(dark))
    }

    pub fn body_muted(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(14.0)
            .color(ThemeColors::text_muted(dark))
    }

    pub fn small(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(12.0)
            .color(ThemeColors::text_primary(dark))
    }

    pub fn small_muted(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(12.0)
            .color(ThemeColors::text_muted(dark))
    }

    pub fn label_strong(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(11.0)
            .strong()
            .color(ThemeColors::text_primary(dark))
    }

    pub fn label_muted(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(11.0)
            .color(ThemeColors::text_muted(dark))
    }
}
