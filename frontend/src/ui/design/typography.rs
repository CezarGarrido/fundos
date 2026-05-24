use super::colors::ThemeColors;
use super::sizing::Scale;
use egui::RichText;

pub struct Typography;

impl Typography {
    pub fn heading_1(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(Scale::DEFAULT.heading_1())
            .strong()
            .color(ThemeColors::header(dark))
    }

    pub fn heading_2(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(Scale::DEFAULT.heading_2())
            .strong()
            .color(ThemeColors::header(dark))
    }

    pub fn heading_3(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(Scale::DEFAULT.heading_3())
            .strong()
            .color(ThemeColors::header(dark))
    }

    pub fn body(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(Scale::DEFAULT.body())
            .color(ThemeColors::text_primary(dark))
    }

    pub fn body_strong(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(Scale::DEFAULT.body())
            .strong()
            .color(ThemeColors::text_primary(dark))
    }

    pub fn body_muted(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(Scale::DEFAULT.body())
            .color(ThemeColors::text_muted(dark))
    }

    pub fn small(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(Scale::DEFAULT.small_text())
            .color(ThemeColors::text_primary(dark))
    }

    pub fn small_muted(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(Scale::DEFAULT.small_text())
            .color(ThemeColors::text_muted(dark))
    }

    pub fn label_strong(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(Scale::DEFAULT.label())
            .strong()
            .color(ThemeColors::text_primary(dark))
    }

    pub fn label_muted(text: impl Into<String>, dark: bool) -> RichText {
        RichText::new(text)
            .size(Scale::DEFAULT.label())
            .color(ThemeColors::text_muted(dark))
    }
}
