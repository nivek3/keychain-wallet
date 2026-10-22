use crate::{macos::{self, Clipboard, ClipboardGate, SecureInput, Vault}, model::*};
use eframe::egui::{self, Event, Id, TextEdit, ViewportCommand};
use crate::theme;
use std::{cell::Cell, rc::Rc, sync::atomic::Ordering, time::{Duration, Instant}};
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage { Welcome, Details, Secrets, Reveal, Backup, Confirm, Success, Delete, Fatal }
struct WalletApp {
    vault: Vault,
    stage: Stage,
    creating: bool,
    name: String,
    path: String,
    curve: Curve,
    expected: String,
    mnemonic: Zeroizing<String>,
    passphrase: Zeroizing<String>,
    repeated_passphrase: Zeroizing<String>,
    backup_confirmation: Zeroizing<String>,
    no_passphrase: bool,
    pending: Option<PendingWallet>,
    public_key: String,
    identifier: String,
    message: String,
    copied: bool,
    clipboard: Clipboard,
    gate: ClipboardGate,
    secure_input: SecureInput,
    cli_import: bool,
    cli_result: Rc<Cell<Option<Result<()>>>>,
}

fn secret_field(ui: &mut egui::Ui, label: &str, id: &str, value: &mut String) -> egui::Response {
    theme::label(ui,label);
    let mut output = theme::input_frame().show(ui,|ui| {
        TextEdit::singleline(value).id(Id::new(id)).password(true).frame(false)
            .margin(egui::Margin::ZERO).desired_width(f32::INFINITY).char_limit(4096).show(ui)
    }).inner;
    // Undo history would otherwise retain additional plaintext copies of secrets.
    output.state.clear_undoer();
    output.state.store(ui.ctx(), output.response.id);
    output.response
}
fn filter_input(raw: &mut egui::RawInput) {
    raw.dropped_files.clear();
    raw.hovered_files.clear();
    raw.events.retain(|event| !matches!(event, Event::Copy | Event::Cut | Event::Paste(_)));
}
impl WalletApp {
    fn request(&self) -> KeyRequest { KeyRequest { name: self.name.clone(), path: self.path.clone(), curve: self.curve } }
    fn wipe_draft(&mut self) {
        self.mnemonic.zeroize(); self.passphrase.zeroize(); self.repeated_passphrase.zeroize();
        self.backup_confirmation.zeroize(); self.pending = None;
        self.gate.allow_paste.store(false,Ordering::SeqCst);
        self.gate.paste_requested.store(false,Ordering::SeqCst);
    }
    fn back(&mut self) {
        self.wipe_draft(); self.message.clear(); self.stage = Stage::Welcome;
    }
    fn prepare(&mut self) -> Result<()> {
        let request = self.request();
        request.validate()?;
        check_passphrase(&self.passphrase,&self.repeated_passphrase,self.no_passphrase)?;
        let words = if self.creating { macos::generate_mnemonic()? } else { normalize(&self.mnemonic) };
        let wallet = StoredWallet { version: 1, mnemonic: words.to_string(), passphrase: self.passphrase.to_string() };
        let pair = derive(&wallet,&request,if self.creating { None } else { Some(&self.expected) })?;
        self.public_key = pair.public_key; self.identifier = pair.identifier;
        self.mnemonic.zeroize(); self.passphrase.zeroize(); self.repeated_passphrase.zeroize();
        self.pending = Some(PendingWallet::new(wallet,self.creating));
        if self.creating { self.stage = Stage::Reveal; Ok(()) } else { self.save() }
    }
    fn save(&mut self) -> Result<()> {
        self.vault.insert(&self.name,self.pending.as_ref().ok_or(Failure::Input)?.for_save()?)?;
        self.wipe_draft();
        let metadata = if self.curve == Curve::Secp256k1 {
            serde_json::json!({"name":self.name,"curve":self.curve.name(),"path":self.path,"publicKey":self.public_key,"address":self.identifier})
        } else {
            serde_json::json!({"name":self.name,"curve":self.curve.name(),"path":self.path,"publicKey":self.public_key})
        };
        crate::emit(&metadata)?;
        self.message.clear(); self.stage = Stage::Success;
        self.cli_result.set(Some(Ok(())));
        Ok(())
    }
    fn paste(&mut self) -> Result<()> {
        if self.stage != Stage::Confirm { return Err(Failure::Input); }
        let text = self.clipboard.paste_confirmation()?;
        self.backup_confirmation.clear();
        self.backup_confirmation.push_str(&text);
        Ok(())
    }
    fn title(&self) -> &'static str {
        match self.stage {
            Stage::Welcome => "Welcome to Keychain Wallet",
            Stage::Details => if self.creating { "Name your new wallet" } else { "Choose the wallet to import" },
            Stage::Secrets => if self.creating { "Create a new wallet" } else { "Import an existing wallet" },
            Stage::Reveal => "Secure your wallet", Stage::Backup => "Back up your recovery phrase",
            Stage::Confirm => "Confirm your recovery phrase", Stage::Success => if self.creating { "Wallet created" } else { "Wallet imported" },
            Stage::Delete => "Delete wallet", Stage::Fatal => "Unable to open the wallet",
        }
    }
    fn content(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, focused: bool) {
            match self.stage {
                Stage::Welcome => {
                    ui.heading("Welcome to Keychain Wallet");
                    theme::hint(ui,"Create a new wallet or restore an existing recovery phrase.");
                    ui.add_space(12.0);
                    let mut choice = None;
                    if theme::entry(ui,"New wallet","Generate a recovery phrase and back it up offline.","Create a new wallet",true) { choice = Some(true); }
                    if theme::entry(ui,"Existing wallet","Restore your recovery phrase and verify the account.","Import an existing wallet",false) { choice = Some(false); }
                    if let Some(creating) = choice {
                        self.creating = creating; self.no_passphrase = false; self.copied = false;
                        self.name.clear(); self.expected.clear(); self.curve = Curve::Secp256k1;
                        self.path = "m/44'/60'/0'/0/0".into(); self.message.clear(); self.stage = Stage::Details;
                    }
                    ui.add_space(12.0);
                    theme::hint(ui,"Recovery phrases stay in this Mac's Keychain. Keep an independent offline backup.");
                    if theme::secondary(ui,"Quit").clicked() { ctx.send_viewport_cmd(ViewportCommand::Close); }
                }
                Stage::Details => {
                    theme::label(ui,"Key algorithm");
                    let previous = self.curve;
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut self.curve,Curve::Secp256k1,"secp256k1 (Ethereum)");
                        ui.radio_value(&mut self.curve,Curve::Ed25519,"Ed25519 (SLIP-0010)");
                    });
                    if self.curve != previous {
                        self.expected.clear();
                        self.path = if self.curve == Curve::Secp256k1 { "m/44'/60'/0'/0/0".into() } else { String::new() };
                    }
                    theme::public_field(ui,"Wallet name",&mut self.name,64);
                    theme::hint(ui,"Letters, digits, dots, underscores, and hyphens. Start with a letter or digit.");
                    theme::public_field(ui,"Derivation path",&mut self.path,128);
                    if self.curve == Curve::Ed25519 { ui.label("Every level must be hardened. Example: m/44'/501'/0'/0'. Use your wallet's exact path."); }
                    if !self.creating {
                        theme::public_field(ui,if self.curve == Curve::Secp256k1 { "Expected Ethereum address" } else { "Expected Ed25519 public key (0x hex, 32 bytes)" },&mut self.expected,66);
                        theme::hint(ui,"Use a value from your existing wallet or trusted backup to verify the restored account.");
                    }
                    let (next,back) = theme::actions(ui,"Back","Continue");
                    if next {
                        let result = if self.creating { self.request().validate() } else { self.request().validate_identifier(&self.expected) };
                        match result { Ok(()) => { self.message.clear(); self.stage = Stage::Secrets; }, Err(e) => self.message = e.message().into() }
                    }
                    if back { self.back(); }
                }
                Stage::Secrets => {
                    theme::hint(ui,&format!("{}  /  {}  /  {}",self.name,self.curve.name(),self.path));
                    theme::note(ui,"This optional passphrase changes the wallet you derive. It is not your Mac or wallet unlock password. Spaces are preserved.");
                    if focused {
                        if !self.creating { secret_field(ui,"English secret recovery phrase","mnemonic",&mut self.mnemonic); }
                        secret_field(ui,"BIP-39 passphrase (optional)","passphrase",&mut self.passphrase);
                        secret_field(ui,"Confirm BIP-39 passphrase","repeat-passphrase",&mut self.repeated_passphrase);
                        ui.checkbox(&mut self.no_passphrase,"I use no BIP-39 passphrase");
                        theme::hint(ui,"Clipboard actions are disabled on this page.");
                        ui.add_space(8.0);
                    } else { ui.label("Focus this window to enter wallet secrets."); }
                    let (next,cancel) = theme::actions(ui,"Cancel",if self.creating { "Continue to backup" } else { "Verify and save" });
                    if next && focused {
                        if let Err(error) = self.prepare() { self.message = error.message().into(); }
                    }
                    if cancel {
                        if self.cli_import { self.cli_result.set(Some(Err(Failure::Cancelled))); ctx.send_viewport_cmd(ViewportCommand::Close); }
                        self.back();
                    }
                }
                Stage::Reveal => {
                    theme::note(ui,"Anyone with this phrase can access your funds. This app cannot recover it for you.");
                    ui.label("Write all 12 words down in order and keep them offline. Reveal the phrase only when no one else can see your screen.");
                    ui.add_space(14.0);
                    let (reveal,cancel) = theme::actions(ui,"Cancel","Reveal recovery phrase");
                    if reveal { self.message.clear(); self.stage = Stage::Backup; }
                    if cancel { self.back(); }
                }
                Stage::Backup => {
                    if focused {
                        if let Some(pending) = &self.pending {
                            let cell_width = (ui.available_width()-24.0)/3.0;
                            egui::Grid::new("recovery-words").num_columns(3).spacing([10.0,10.0]).show(ui,|ui| {
                                for (i,word) in pending.phrase().split_whitespace().enumerate() {
                                    egui::Frame::new().fill(theme::CANVAS).corner_radius(6).inner_margin(egui::Margin::symmetric(10,8)).show(ui,|ui| {
                                        ui.set_width((cell_width-24.0).max(60.0));
                                        ui.label(egui::RichText::new(format!("{:02}  {}",i+1,word)).monospace().size(13.0));
                                    });
                                    if i % 3 == 2 { ui.end_row(); }
                                }
                            });
                            ui.add_space(16.0);
                            if theme::secondary(ui,if self.copied { "Copied" } else { "Copy recovery phrase" }).clicked() {
                                match self.clipboard.copy(pending.phrase()) { Ok(()) => self.copied = true, Err(e) => self.message = e.message().into() }
                            }
                        }
                        theme::hint(ui,"Other apps may keep clipboard copies. This app clears its copy after 60 seconds while running, unless you copy something else.");
                    } else { ui.label("Recovery phrase hidden while this window is inactive."); }
                    let (next,cancel) = theme::actions(ui,"Cancel","I have saved my recovery phrase");
                    if next && focused { self.message.clear(); self.stage = Stage::Confirm; }
                    if cancel { self.back(); }
                }
                Stage::Confirm => {
                    ui.label("Enter all 12 words in order to check your backup. Your wallet is saved only after they match.");
                    if focused {
                        let response = secret_field(ui,"Secret recovery phrase","backup-confirmation",&mut self.backup_confirmation);
                        self.gate.allow_paste.store(response.has_focus(),Ordering::SeqCst);
                        if theme::secondary(ui,"Paste recovery phrase").clicked() {
                            if let Err(e) = self.paste() { self.message = e.message().into(); }
                            response.request_focus();
                        }
                    }
                    let (save,cancel) = theme::actions(ui,"Cancel","Confirm and save");
                    if save && focused {
                        let result = self.pending.as_mut().ok_or(Failure::Input).and_then(|p| p.confirm(&self.backup_confirmation)).and_then(|_| self.save());
                        if let Err(e) = result { self.message = if e == Failure::Input { "Recovery phrase does not match. Check all 12 words in order. Nothing was saved.".into() } else { e.message().into() }; }
                    }
                    if cancel { self.back(); }
                }
                Stage::Success => {
                    theme::note(ui,"Saved in Keychain. Your application will ask for macOS authentication before reading a key.");
                    theme::hint(ui,&format!("{}  /  {}  /  {}",self.name,self.curve.name(),self.path));
                    theme::label(ui,"Public key");
                    ui.label(egui::RichText::new(&self.public_key).monospace());
                    if self.curve == Curve::Secp256k1 { theme::label(ui,"Address"); ui.label(egui::RichText::new(&self.identifier).monospace()); }
                    if self.cli_import {
                        if theme::primary(ui,"Done").clicked() { ctx.send_viewport_cmd(ViewportCommand::Close); }
                    } else if theme::primary(ui,"Back to welcome").clicked() { self.back(); }
                }
                Stage::Delete => {
                    ui.label(format!("Permanently delete wallet: {}?",self.name));
                    ui.label("Continue only if you have an independent offline backup. This tool cannot recover a deleted wallet.");
                    let (delete,cancel) = theme::actions(ui,"Cancel","Authenticate and delete");
                    if cancel { self.cli_result.set(Some(Err(Failure::Cancelled))); ctx.send_viewport_cmd(ViewportCommand::Close); }
                    if delete {
                        let context = macos::AuthContext::new(&format!("Delete wallet {}",self.name),false);
                        let result = self.vault.read(&self.name,&context).and_then(|_| self.vault.delete(&self.name,&context))
                            .and_then(|_| crate::emit(&serde_json::json!({"name":self.name})));
                        match result {
                            Ok(()) => { self.cli_result.set(Some(Ok(()))); ctx.send_viewport_cmd(ViewportCommand::Close); }
                            Err(e) => self.message = e.message().into(),
                        }
                    }
                }
                Stage::Fatal => { ui.label(&self.message); if theme::secondary(ui,"Quit").clicked() { ctx.send_viewport_cmd(ViewportCommand::Close); } }
            }
            if self.stage != Stage::Fatal && !self.message.is_empty() { ui.add_space(12.0); egui::Frame::new().fill(egui::Color32::from_rgb(255,241,240)).corner_radius(8).inner_margin(12).show(ui,|ui| { ui.colored_label(egui::Color32::from_rgb(168,48,45),&self.message); }); }
    }

}

impl eframe::App for WalletApp {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) { filter_input(raw); }
    fn persist_egui_memory(&self) -> bool { false }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let focused = ctx.input(|i| i.focused);
        let contains_secrets = matches!(self.stage,Stage::Secrets | Stage::Reveal | Stage::Backup | Stage::Confirm);
        if let Err(error) = self.secure_input.set(focused && contains_secrets) {
            self.wipe_draft(); self.cli_result.set(Some(Err(error))); self.message = error.message().into(); self.stage = Stage::Fatal;
        }
        self.clipboard.tick(Instant::now());
        ctx.request_repaint_after(Duration::from_millis(250));
        if self.gate.paste_requested.swap(false,Ordering::SeqCst) && self.stage == Stage::Confirm && focused {
            if let Err(error) = self.paste() { self.message = error.message().into(); }
        }
        self.gate.allow_paste.store(false,Ordering::SeqCst);
        egui::TopBottomPanel::top("brand").frame(egui::Frame::new().fill(egui::Color32::WHITE).inner_margin(egui::Margin::symmetric(20,12))).show(ctx,theme::brand);
        egui::TopBottomPanel::bottom("privacy-footer").frame(egui::Frame::new().fill(theme::CANVAS).inner_margin(egui::Margin::symmetric(20,10))).show(ctx,|ui| {
            ui.horizontal(|ui| {
                theme::hint(ui,"Protected by macOS Keychain");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| { theme::hint(ui,"No cloud sync  ·  Keep an offline backup"); });
            });
        });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::CANVAS).inner_margin(egui::Margin::same(20))).show(ctx,|ui| {
            let height = ui.available_height();
            if self.stage == Stage::Welcome {
                theme::card().show(ui,|ui| {
                    ui.set_width(ui.available_width());
                    egui::ScrollArea::vertical().id_salt("welcome-scroll").max_height(height-40.0).show(ui,|ui| self.content(ui,ctx,focused));
                });
            } else {
                let width = (ui.available_width()-144.0-20.0-40.0).max(280.0);
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(egui::vec2(144.0,0.0),egui::Layout::top_down(egui::Align::Min),|ui| {
                        ui.set_width(144.0);
                        ui.spacing_mut().item_spacing = egui::vec2(10.0,10.0);
                        let step = match self.stage { Stage::Details => 0, Stage::Secrets => 1, Stage::Reveal | Stage::Backup => 2, Stage::Confirm => 3, _ => 4 };
                        theme::steps(ui,self.creating,step);
                    });
                    ui.add_space(20.0);
                    theme::card().show(ui,|ui| {
                        ui.set_width(width);
                        ui.spacing_mut().item_spacing = egui::vec2(10.0,8.0);
                        ui.with_layout(egui::Layout::top_down(egui::Align::Min),|ui| {
                            egui::ScrollArea::vertical().id_salt("wallet-scroll").max_height(height-40.0).show(ui,|ui| {
                                ui.heading(self.title());
                                ui.add_space(10.0);
                                self.content(ui,ctx,focused);
                            });
                        });
                    });
                });
            }
        });
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.wipe_draft(); let _ = self.secure_input.set(false);
    }
}

pub fn run(vault: Vault, import: Option<(KeyRequest,String)>, delete: Option<String>) -> Result<()> {
    let gate = ClipboardGate::new()?;
    let cli_mode = import.is_some() || delete.is_some();
    let outcome = Rc::new(Cell::new(None));
    let mut app = WalletApp {
        vault, stage: Stage::Welcome, creating: false, name: String::new(), path: "m/44'/60'/0'/0/0".into(),
        curve: Curve::Secp256k1, expected: String::new(), mnemonic: Zeroizing::new(String::new()),
        passphrase: Zeroizing::new(String::new()), repeated_passphrase: Zeroizing::new(String::new()),
        backup_confirmation: Zeroizing::new(String::new()), no_passphrase: false, pending: None,
        public_key: String::new(), identifier: String::new(), message: String::new(), copied: false,
        clipboard: Clipboard::new(), gate, secure_input: SecureInput::new(), cli_import: import.is_some(), cli_result: outcome.clone(),
    };
    if let Some((request,expected)) = import { app.name = request.name; app.path = request.path; app.curve = request.curve; app.expected = expected; app.stage = Stage::Secrets; }
    if let Some(name) = delete { app.name = name; app.stage = Stage::Delete; }
    let options = eframe::NativeOptions { viewport: egui::ViewportBuilder::default().with_inner_size([820.0,640.0]).with_min_inner_size([740.0,600.0]).with_drag_and_drop(false), ..Default::default() };
    eframe::run_native("Keychain Wallet",options,Box::new(move |cc| { theme::configure(&cc.egui_ctx); Ok(Box::new(app)) })).map_err(|_| Failure::Configuration)?;
    if cli_mode { outcome.take().unwrap_or(Err(Failure::Cancelled)) } else { outcome.take().unwrap_or(Ok(())) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipboard_and_file_input_cannot_bypass_explicit_confirmation_paste() {
        let mut raw = egui::RawInput { events: vec![Event::Copy,Event::Cut,Event::Paste("public sentinel".into()),Event::Text("t".into())], ..Default::default() };
        filter_input(&mut raw);
        assert_eq!(raw.events,vec![Event::Text("t".into())]);
    }
}
