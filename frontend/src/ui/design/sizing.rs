/// Fonte única de verdade para todos os tamanhos da UI.
///
/// Todos os tamanhos de fonte derivam de `base` via multiplicadores semânticos.
/// Ajustar `base` escala toda a interface proporcionalmente.
#[derive(Clone, Copy)]
pub struct Scale {
    pub base: f32,
}

impl Default for Scale {
    fn default() -> Self {
        Self { base: 16.0 }
    }
}

impl Scale {
    pub const DEFAULT: Scale = Scale { base: 16.0 };

    // ── Text sizes (multiplicadores da base) ──

    /// 9.0 — rótulo auxiliar mínimo (ex: sub-label de métrica)
    pub fn label_mini(self) -> f32 {
        self.base * 0.5625
    }
    /// 9.5 — descrições muito pequenas
    pub fn description(self) -> f32 {
        self.base * 0.59375
    }
    /// 10.0 — badges e tags
    pub fn badge(self) -> f32 {
        self.base * 0.625
    }
    /// 10.5 — células de tabela de dados
    pub fn table_cell(self) -> f32 {
        self.base * 0.65625
    }
    /// 11.0 — labels e rótulos
    pub fn label(self) -> f32 {
        self.base * 0.6875
    }
    /// 11.5 — legendas
    pub fn caption(self) -> f32 {
        self.base * 0.71875
    }
    /// 12.0 — texto pequeno
    pub fn small_text(self) -> f32 {
        self.base * 0.75
    }
    /// 13.0 — botões
    pub fn button(self) -> f32 {
        self.base * 0.8125
    }
    /// 16.0 — corpo de texto (mínimo WCAG para body text)
    pub fn body(self) -> f32 {
        self.base * 1.0
    }
    /// 18.0 — valores de métrica/KPI
    pub fn metric(self) -> f32 {
        self.base * 1.125
    }
    /// 22.0 — métrica grande (dashboard)
    pub fn metric_large(self) -> f32 {
        self.base * 1.375
    }
    /// 16.0 — heading 3
    pub fn heading_3(self) -> f32 {
        self.base * 1.0
    }
    /// 20.0 — heading 2
    pub fn heading_2(self) -> f32 {
        self.base * 1.25
    }
    /// 26.0 — heading 1
    pub fn heading_1(self) -> f32 {
        self.base * 1.625
    }

    // ── Layout sizes ──

    /// 24.0 — altura de linha de tabela (mínimo WCAG 2.5.8 touch target)
    pub fn table_row_height(self) -> f32 {
        self.base * 1.5
    }
    /// 20.0 — altura compacta para tabelas densas
    pub fn table_row_compact(self) -> f32 {
        self.base * 1.25
    }
    /// 26.0 — altura de header de tabela
    pub fn table_header_height(self) -> f32 {
        self.base * 1.625
    }

    // ── Ícones (fixos, não escalam com fonte) ──

    pub const ICON_SPINNER: f32 = 32.0;
    pub const ICON_SMALL: f32 = 14.0;
    pub const ICON_HERO: f32 = 40.0;
    pub const ICON_LARGE: f32 = 42.0;
    pub const ICON_JUMBO: f32 = 48.0;
    pub const ICON_BRAND: f32 = 52.0;

    // ── Modal / window dimensions (fixas, não escalam) ──

    pub const MODAL_NARROW: f32 = 380.0;
    pub const MODAL_MEDIUM: f32 = 550.0;
    pub const MODAL_WIDE: f32 = 600.0;
    pub const MODAL_WIDTH_KPI: f32 = 200.0;
    pub const MODAL_HEIGHT_KPI: f32 = 350.0;
    pub const COLUMN_MIN: f32 = 200.0;
}

// Compile-time assertion: base nunca pode baixar de 16.0 (mínimo WCAG)
#[allow(dead_code)]
const _ASSERT_BASE_MIN: () =
    assert!(Scale::DEFAULT.base >= 16.0, "Body text below WCAG minimum");
