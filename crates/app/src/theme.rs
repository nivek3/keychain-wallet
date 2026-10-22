use eframe::egui::{self, Align, Color32, FontId, Frame, Layout, Margin, RichText, Stroke, TextStyle, Ui};

pub const CANVAS: Color32 = Color32::from_rgb(245,245,247);
pub const INK: Color32 = Color32::from_rgb(29,29,31);
pub const MUTED: Color32 = Color32::from_rgb(110,110,115);
pub const BORDER: Color32 = Color32::from_rgb(218,218,224);
pub const ACCENT: Color32 = Color32::from_rgb(0,104,217);
pub const SOFT: Color32 = Color32::from_rgb(235,242,252);

pub fn configure(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Ok(bytes) = std::fs::read("/System/Library/Fonts/SFNS.ttf") {
        fonts.font_data.insert("macOS system".into(),egui::FontData::from_owned(bytes).into());
        fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0,"macOS system".into());
    }
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.override_text_color = Some(INK);
    style.visuals.weak_text_color = Some(MUTED);
    style.visuals.panel_fill = CANVAS;
    style.visuals.extreme_bg_color = Color32::WHITE;
    style.visuals.selection.bg_fill = SOFT;
    style.visuals.selection.stroke = Stroke::new(1.0_f32,ACCENT);
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32,BORDER);
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32,ACCENT);
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32,ACCENT);
    style.visuals.widgets.inactive.corner_radius = 6.into();
    style.visuals.widgets.hovered.corner_radius = 6.into();
    style.visuals.widgets.active.corner_radius = 6.into();
    style.spacing.item_spacing = egui::vec2(10.0,8.0);
    style.spacing.button_padding = egui::vec2(14.0,7.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.icon_width = 16.0;
    style.text_styles.insert(TextStyle::Heading,FontId::proportional(21.0));
    style.text_styles.insert(TextStyle::Body,FontId::proportional(13.0));
    style.text_styles.insert(TextStyle::Button,FontId::proportional(13.0));
    style.text_styles.insert(TextStyle::Small,FontId::proportional(11.0));
    style.text_styles.insert(TextStyle::Monospace,FontId::monospace(12.0));
    ctx.set_style(style);
}
pub fn card() -> Frame { Frame::new().fill(Color32::WHITE).stroke(Stroke::new(1.0_f32,BORDER)).corner_radius(10).inner_margin(Margin::same(20)) }
pub fn primary(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(label).color(Color32::WHITE)).min_size(egui::vec2(136.0,32.0)).fill(ACCENT).stroke(Stroke::NONE).corner_radius(6))
}
pub fn secondary(ui: &mut Ui, label: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(label).color(INK)).min_size(egui::vec2(76.0,32.0)).fill(Color32::WHITE).stroke(Stroke::new(1.0_f32,BORDER)).corner_radius(6))
}
pub fn actions(ui: &mut Ui, back: &str, next: &str) -> (bool,bool) {
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);
    let mut previous = false;
    let mut proceed = false;
    ui.horizontal(|ui| {
        previous = secondary(ui,back).clicked();
        ui.with_layout(Layout::right_to_left(Align::Center),|ui| { proceed = primary(ui,next).clicked(); });
    });
    (proceed,previous)
}
pub fn hint(ui: &mut Ui, text: &str) { ui.label(RichText::new(text).size(11.0).color(MUTED)); }
pub fn note(ui: &mut Ui, text: &str) {
    Frame::new().fill(SOFT).corner_radius(6).inner_margin(Margin::same(10)).show(ui, |ui| {
        ui.set_width((ui.available_width()-2.0).max(10.0));
        ui.label(RichText::new(text).size(12.0).color(INK));
    });
}
pub fn label(ui: &mut Ui, text: &str) { ui.label(RichText::new(text).size(12.0).strong()); }
pub fn input_frame() -> Frame { Frame::new().fill(Color32::WHITE).stroke(Stroke::new(1.0_f32,BORDER)).corner_radius(6).inner_margin(Margin::symmetric(10,8)) }
pub fn public_field(ui: &mut Ui, title: &str, text: &mut String, limit: usize) {
    label(ui,title);
    input_frame().show(ui,|ui| {
        ui.add(egui::TextEdit::singleline(text).char_limit(limit).frame(false).margin(Margin::ZERO).desired_width(f32::INFINITY));
    });
}
pub fn brand(ui: &mut Ui) {
    ui.horizontal(|ui| {
        let (rect,_) = ui.allocate_exact_size(egui::vec2(28.0,28.0),egui::Sense::hover());
        let c = rect.center();
        ui.painter().rect_filled(rect,7,SOFT);
        ui.painter().circle_stroke(c+egui::vec2(-3.0,-3.0),4.0,Stroke::new(1.8_f32,ACCENT));
        ui.painter().line_segment([c,c+egui::vec2(7.0,7.0)],Stroke::new(1.8_f32,ACCENT));
        ui.painter().line_segment([c+egui::vec2(4.0,4.0),c+egui::vec2(7.0,1.0)],Stroke::new(1.8_f32,ACCENT));
        ui.label(RichText::new("Keychain Wallet").size(16.0).strong());
    });
}
pub fn steps(ui: &mut Ui, creating: bool, current: usize) {
    hint(ui,if creating { "New wallet" } else { "Restore wallet" });
    ui.add_space(8.0);
    let steps: &[&str] = if creating { &["Wallet details","Passphrase","Back up","Confirm"] } else { &["Wallet details","Recovery phrase","Verify & save"] };
    for (i,title) in steps.iter().enumerate() {
        ui.horizontal(|ui| {
            let (rect,_) = ui.allocate_exact_size(egui::vec2(22.0,22.0),egui::Sense::hover());
            let active = i == current;
            ui.painter().circle_filled(rect.center(),11.0,if active { ACCENT } else { BORDER });
            ui.painter().text(rect.center(),egui::Align2::CENTER_CENTER,(i+1).to_string(),FontId::proportional(11.0),if active { Color32::WHITE } else { MUTED });
            ui.label(RichText::new(*title).size(12.0).color(if active { INK } else { MUTED }));
        });
        ui.add_space(8.0);
    }
}
pub fn entry(ui: &mut Ui, title: &str, description: &str, button: &str, fresh: bool) -> bool {
    let width = ui.available_width();
    card().inner_margin(Margin::same(16)).show(ui,|ui| {
        ui.set_width((width-32.0).max(80.0));
        let mut clicked = false;
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(egui::vec2((ui.available_width()-200.0).max(120.0),58.0),Layout::top_down(Align::Min),|ui| {
                ui.label(RichText::new(title).size(15.0).strong());
                hint(ui,description);
            });
            ui.with_layout(Layout::right_to_left(Align::Center),|ui| {
                clicked = if fresh { primary(ui,button).clicked() } else { secondary(ui,button).clicked() };
            });
        });
        clicked
    }).inner
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn system_font_and_form_render_without_overflow() {
        let ctx = egui::Context::default();
        configure(&ctx);
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(740.0,600.0))), ..Default::default() };
        let mut text = "public form fixture".to_owned();
        let _ = ctx.run(raw,|ctx| {
            egui::CentralPanel::default().show(ctx,|ui| {
                let bounds = ui.max_rect();
                let card = card().show(ui,|ui| {
                    ui.set_max_width(400.0);
                    ui.heading("Name your new wallet");
                    public_field(ui,"Wallet name",&mut text,64);
                    let button = primary(ui,"Continue");
                    assert!(bounds.contains_rect(button.rect));
                    assert!(button.rect.width() <= 200.0);
                    assert!(button.rect.height() <= 36.0);
                });
                assert!(bounds.contains_rect(card.response.rect));
            });
        });
    }
}
