// Select input node — drop-down field.
//
// Up/Down cycles through options internally.
// The dropdown list is rendered via render_overlay(), called after all regular
// fields so it visually appears on top.
//
// Options are `Vec<String>` so both static (&'static str) and dynamic
// (runtime-computed, e.g. project slugs) choices are supported.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::app::Lang;
use crate::ui::form_node::{FormAction, FormNode};

#[derive(Debug)]
pub struct SelectInputNode {
    pub key:        &'static str,
    pub label_key:  &'static str,
    pub hint_key:   Option<&'static str>,
    pub tab:        usize,
    pub required:   bool,
    pub value:      String,
    /// Available choices. `Vec<String>` supports both static and runtime-computed options.
    pub options:    Vec<String>,
    /// Maps an option code to a human-readable label.
    pub display_fn: Option<fn(&str) -> &'static str>,
    rect:           Option<Rect>,
}

impl SelectInputNode {
    pub fn new(
        key:       &'static str,
        label_key: &'static str,
        tab:       usize,
        required:  bool,
        options:   Vec<String>,
    ) -> Self {
        let value = options.first().cloned().unwrap_or_default();
        Self {
            key, label_key, hint_key: None, tab, required,
            value, options, display_fn: None, rect: None,
        }
    }

    // ── Builder helpers ────────────────────────────────────────────────────

    pub fn hint(mut self, k: &'static str) -> Self { self.hint_key = Some(k); self }

    pub fn default_val(mut self, v: &str) -> Self {
        self.value = v.to_string();
        self
    }

    pub fn display(mut self, f: fn(&str) -> &'static str) -> Self {
        self.display_fn = Some(f);
        self
    }

    // ── Internal helpers ───────────────────────────────────────────────────

    fn current_idx(&self) -> usize {
        self.options.iter().position(|o| o == &self.value).unwrap_or(0)
    }

    fn next_option(&mut self) {
        if self.options.is_empty() { return; }
        let next = (self.current_idx() + 1) % self.options.len();
        self.value = self.options[next].clone();
    }

    fn prev_option(&mut self) {
        if self.options.is_empty() { return; }
        let cur  = self.current_idx();
        let prev = if cur == 0 { self.options.len() - 1 } else { cur - 1 };
        self.value = self.options[prev].clone();
    }

    fn human_label(&self) -> &str {
        if let Some(f) = self.display_fn {
            let s = f(&self.value);
            if !s.is_empty() { return s; }
        }
        &self.value
    }

    fn set_by_index(&mut self, idx: usize) {
        if let Some(opt) = self.options.get(idx) {
            self.value = opt.clone();
        }
    }
}

impl FormNode for SelectInputNode {
    fn key(&self)       -> &'static str         { self.key }
    fn label_key(&self) -> &'static str         { self.label_key }
    fn hint_key(&self)  -> Option<&'static str> { self.hint_key }
    fn tab(&self)       -> usize                { self.tab }
    fn required(&self)  -> bool                 { self.required }

    fn value(&self)           -> &str { &self.value }
    fn effective_value(&self) -> &str { &self.value }  // Select always has a valid value

    fn set_value(&mut self, v: &str) { self.value = v.to_string(); }
    fn is_dirty(&self)  -> bool      { false }   // Select is never "dirty"
    fn set_dirty(&mut self, _v: bool) {}

    fn set_rect(&mut self, r: Rect)     { self.rect = Some(r); }
    fn last_rect(&self) -> Option<Rect> { self.rect }

    fn render(&mut self, f: &mut Frame, area: Rect, focused: bool, lang: Lang) {
        self.set_rect(area);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // label
                Constraint::Length(3), // input box
                Constraint::Length(1), // hint (hidden when focused)
            ])
            .split(area);

        // Label
        let req_marker = if self.required {
            Span::styled(" *", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        } else {
            Span::raw("")
        };
        let label_style = if focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(crate::i18n::t(lang, self.label_key), label_style),
                req_marker,
            ])),
            rows[0],
        );

        // Input box — shows current selection + cursor when focused
        let border_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let display = self.human_label();
        let input_line = if focused {
            Line::from(vec![
                Span::styled(display.to_string(), Style::default().fg(Color::White)),
                Span::styled("█",                Style::default().fg(Color::Cyan)),
            ])
        } else {
            Line::from(Span::styled(display.to_string(), Style::default().fg(Color::White)))
        };
        f.render_widget(
            Paragraph::new(input_line)
                .block(Block::default().borders(Borders::ALL).border_style(border_style)),
            rows[1],
        );

        // Hint — hidden when focused (dropdown takes that space)
        if !focused {
            if let Some(hk) = self.hint_key {
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        crate::i18n::t(lang, hk),
                        Style::default().fg(Color::DarkGray),
                    ))),
                    rows[2],
                );
            }
        }
    }

    /// Render the dropdown list below the input box.
    /// `available` is the total form area — limits how tall the dropdown can grow.
    fn render_overlay(&mut self, f: &mut Frame, available: Rect, _lang: Lang) {
        let Some(input_rect) = self.last_rect() else { return };
        let input_box_bottom = input_rect.y + 1 + 3; // label(1) + box(3)
        let avail_h = available.bottom().saturating_sub(input_box_bottom);
        let want_h  = (self.options.len() as u16 + 2).min(avail_h);
        if want_h < 3 { return; }

        let dropdown = Rect {
            x: input_rect.x,
            y: input_box_bottom,
            width: input_rect.width,
            height: want_h,
        };
        let cur = self.current_idx();

        let items: Vec<ListItem> = self.options.iter().enumerate().map(|(i, opt)| {
            let label  = if let Some(f) = self.display_fn { f(opt.as_str()) } else { opt.as_str() };
            let prefix = if i == cur { "▶ " } else { "  " };
            let style  = if i == cur {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(format!("{}{}", prefix, label), style)))
        }).collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)));

        f.render_widget(Clear, dropdown);
        f.render_widget(list, dropdown);
    }

    fn handle_key(&mut self, key: KeyEvent) -> FormAction {
        use KeyModifiers as KM;
        match key.code {
            // Selection — handled internally
            KeyCode::Up   => { self.prev_option(); FormAction::Consumed }
            KeyCode::Down => { self.next_option(); FormAction::Consumed }
            // Enter confirms the current selection and advances to the next field,
            // matching TextInputNode behaviour so the user can navigate with Enter.
            KeyCode::Enter => FormAction::FocusNext,

            // Navigation
            KeyCode::Tab     => FormAction::FocusNext,
            KeyCode::BackTab => FormAction::FocusPrev,
            KeyCode::Esc     => FormAction::Cancel,
            KeyCode::Left  if key.modifiers.contains(KM::CONTROL) => FormAction::TabPrev,
            KeyCode::Right if key.modifiers.contains(KM::CONTROL) => FormAction::TabNext,

            // Language toggle
            KeyCode::Char('l') | KeyCode::Char('L') => FormAction::LangToggle,

            _ => FormAction::Unhandled,
        }
    }
}

// ── Mouse click support ───────────────────────────────────────────────────────

impl SelectInputNode {
    /// If a mouse click lands on the dropdown list, set the option and return true.
    pub fn click_dropdown(&mut self, col: u16, row: u16, available: Rect) -> bool {
        let Some(input_rect) = self.last_rect() else { return false };
        let input_box_bottom = input_rect.y + 1 + 3;
        let avail_h = available.bottom().saturating_sub(input_box_bottom);
        let want_h  = (self.options.len() as u16 + 2).min(avail_h);
        if want_h < 3 { return false; }

        let dropdown = Rect {
            x: input_rect.x,
            y: input_box_bottom,
            width: input_rect.width,
            height: want_h,
        };
        if col < dropdown.x || col >= dropdown.right() { return false; }
        if row <= dropdown.y || row >= dropdown.bottom() { return false; }
        let item_row = (row - dropdown.y - 1) as usize;
        self.set_by_index(item_row);
        true
    }
}
