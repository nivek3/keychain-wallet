mod wallets;

use crate::catalog::Catalog;
use crate::theme;
use crate::{
    macos::{self, Clipboard, ClipboardGate, SecureInput, Vault},
    model::*,
};
use eframe::egui::{self, Event, Id, TextEdit, ViewportCommand};
use std::{
    cell::Cell,
    rc::Rc,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Welcome,
    Wallets,
    Details,
    Secrets,
    Reveal,
    Backup,
    Confirm,
    Success,
    ViewPhrase,
    Delete,
    Fatal,
}
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
    revealed: PhraseReveal,
    recovery_card_height: f32,
    public_key: String,
    identifier: String,
    message: String,
    copied: bool,
    clipboard: Clipboard,
    gate: ClipboardGate,
    secure_input: SecureInput,
    cli_import: bool,
    cli_delete: bool,
    wallets: Vec<WalletSummary>,
    search: String,
    selected_wallet: Option<String>,
    show_wallet_details: bool,
    deletion: Option<DeletePlan>,
    cli_result: Rc<Cell<Option<Result<()>>>>,
}

#[derive(Default)]
struct PhraseReveal {
    phrase: Zeroizing<String>,
    deadline: Option<Instant>,
}
impl PhraseReveal {
    fn show(&mut self, phrase: String, now: Instant) {
        self.clear();
        self.phrase = Zeroizing::new(phrase);
        self.deadline = Some(now + Duration::from_secs(60));
    }
    fn clear(&mut self) {
        self.phrase.zeroize();
        self.deadline = None;
    }
    fn visible(&mut self, now: Instant, focused: bool) -> Option<&str> {
        if !focused || self.deadline.is_none_or(|deadline| now >= deadline) {
            self.clear();
            return None;
        }
        Some(&self.phrase)
    }
}

fn recovery_words(ui: &mut egui::Ui, phrase: &str) {
    let columns = if ui.available_width() < 400.0 { 2 } else { 3 };
    let cell_width = ((ui.available_width() - 10.0 * (columns - 1) as f32) / columns as f32).floor();
    egui::Grid::new("recovery-words")
        .num_columns(columns)
        .spacing([10.0, 10.0])
        .show(ui, |ui| {
            for (i, word) in phrase.split_whitespace().enumerate() {
                egui::Frame::new()
                    .fill(theme::CANVAS)
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.set_width((cell_width - 20.0).max(60.0));
                        ui.add(egui::Label::new(egui::RichText::new(format!("{:02}  {}", i + 1, word))
                            .monospace().size(13.0)).selectable(false));
                    });
                if (i + 1) % columns == 0 {
                    ui.end_row();
                }
            }
        });
}

fn revealed_card(ui: &mut egui::Ui, phrase: &str, height: f32) -> egui::Response {
    let width = ui.available_width();
    egui::Frame::new().fill(egui::Color32::WHITE).corner_radius(10).inner_margin(16)
        .show(ui, |ui| {
            ui.set_width((width - 32.0).max(0.0));
            ui.set_min_height((height - 32.0).max(0.0));
            recovery_words(ui, phrase);
        }).response
}

fn secret_field(ui: &mut egui::Ui, label: &str, id: &str, value: &mut String) -> egui::Response {
    theme::label(ui, label);
    let field = theme::input_frame().show(ui, |ui| {
        TextEdit::singleline(value)
            .id(Id::new(id))
            .password(true)
            .frame(false)
            .margin(egui::Margin::ZERO)
            .desired_width(f32::INFINITY)
            .char_limit(4096)
            .show(ui)
    });
    theme::focus_ring(ui, field.response.rect, field.inner.response.has_focus());
    let mut output = field.inner;
    // Undo history would otherwise retain additional plaintext copies of secrets.
    output.state.clear_undoer();
    output.state.store(ui.ctx(), output.response.id);
    output.response
}
fn filter_input(raw: &mut egui::RawInput) {
    raw.dropped_files.clear();
    raw.hovered_files.clear();
    raw.events
        .retain(|event| !matches!(event, Event::Copy | Event::Cut | Event::Paste(_)));
}

fn saved_message(indexed: bool, output: Result<()>) -> String {
    let mut message = String::new();
    if !indexed {
        message.push_str("Wallet saved in Keychain, but the local list could not be updated. Use Sync from Keychain to repair it.");
    }
    if output.is_err() {
        if !message.is_empty() {
            message.push(' ');
        }
        message.push_str("Wallet saved in Keychain, but the command-line result could not be delivered. Do not import it again.");
    }
    message
}

impl WalletApp {
    fn prepare_authentication(&mut self) {
        self.revealed.clear();
        // Authentication can block the UI timer; clear our sensitive copy first.
        self.clipboard.clear();
        self.copied = false;
    }
    fn request(&self) -> KeyRequest {
        KeyRequest {
            name: self.name.clone(),
            path: self.path.clone(),
            curve: self.curve,
        }
    }
    fn wipe_draft(&mut self) {
        self.revealed.clear();
        self.mnemonic.zeroize();
        self.passphrase.zeroize();
        self.repeated_passphrase.zeroize();
        self.backup_confirmation.zeroize();
        self.pending = None;
        self.gate.allow_paste.store(false, Ordering::SeqCst);
        self.gate.paste_requested.store(false, Ordering::SeqCst);
    }
    fn reveal_wallet(&mut self, name: &str) -> Result<()> {
        self.wipe_draft();
        self.prepare_authentication();
        self.secure_input.set(true)?;
        let context = macos::AuthContext::new(
            &format!("Show recovery phrase for wallet: {name}"), false,
        );
        let mut wallet = match self.vault.read(name, &context) {
            Ok(wallet) => wallet,
            Err(error) => {
                let _ = self.secure_input.set(false);
                return Err(error);
            }
        };
        self.revealed.show(std::mem::take(&mut wallet.mnemonic), Instant::now());
        self.name = name.to_owned();
        self.show_wallet_details = true;
        self.message.clear();
        Ok(())
    }
    fn back(&mut self) {
        self.wipe_draft();
        self.show_wallets();
    }
    fn return_to_wallets(&mut self) {
        self.revealed.clear();
        self.deletion = None;
        self.message.clear();
        self.stage = Stage::Wallets;
        self.show_wallet_details = false;
    }
    fn show_wallets(&mut self) {
        self.return_to_wallets();
        self.wallets.clear();
        match Catalog::for_group(&self.vault.group).and_then(|c| c.load()) {
            Ok(entries) => self.wallets = entries,
            Err(error) => {
                self.message = format!(
                "Unable to load the local name list ({}). Use Sync from Keychain to rebuild it.",
                error.code()
            )
            }
        }
    }
    fn sync_wallets(&mut self) {
        self.prepare_authentication();
        match Catalog::for_group(&self.vault.group).and_then(|c| c.sync(|| self.vault.summaries()))
        {
            Ok(entries) => {
                self.wallets = entries;
                self.message.clear();
            }
            Err(error) => {
                self.message = format!(
                    "Name synchronization stopped ({}). The previous list was kept.",
                    error.code()
                )
            }
        }
    }
    fn confirm_delete(&mut self, names: Vec<String>) {
        self.revealed.clear();
        match DeletePlan::new(names) {
            Ok(plan) => {
                self.deletion = Some(plan);
                self.message.clear();
                self.stage = Stage::Delete;
            }
            Err(error) => self.message = format!("Unable to prepare deletion ({}).", error.code()),
        }
    }
    fn prepare(&mut self) -> Result<()> {
        let request = self.request();
        request.validate()?;
        check_passphrase(
            &self.passphrase,
            &self.repeated_passphrase,
            self.no_passphrase,
        )?;
        let words = if self.creating {
            macos::generate_mnemonic()?
        } else {
            normalize(&self.mnemonic)
        };
        let wallet = StoredWallet {
            version: 1,
            mnemonic: words.to_string(),
            passphrase: self.passphrase.to_string(),
        };
        let pair = derive(
            &wallet,
            &request,
            if self.creating {
                None
            } else {
                Some(&self.expected)
            },
        )?;
        self.public_key = pair.public_key;
        self.identifier = pair.identifier;
        self.mnemonic.zeroize();
        self.passphrase.zeroize();
        self.repeated_passphrase.zeroize();
        self.pending = Some(PendingWallet::new(wallet, self.creating));
        if self.creating {
            self.stage = Stage::Reveal;
            Ok(())
        } else {
            self.save()
        }
    }
    fn save(&mut self) -> Result<()> {
        self.prepare_authentication();
        let wallet = self.pending.as_ref().ok_or(Failure::Input)?.for_save()?;
        let summary = WalletSummary::from_wallet(&self.name, wallet)?;
        self.vault.insert(&self.name, wallet)?;
        let indexed = Catalog::for_group(&self.vault.group)
            .and_then(|c| c.upsert(summary))
            .is_ok();
        self.wipe_draft();
        let metadata = if self.curve == Curve::Secp256k1 {
            serde_json::json!({"name":self.name,"curve":self.curve.name(),"path":self.path,"publicKey":self.public_key,"address":self.identifier})
        } else {
            serde_json::json!({"name":self.name,"curve":self.curve.name(),"path":self.path,"publicKey":self.public_key})
        };
        let output = if self.cli_import {
            crate::emit(&metadata)
        } else {
            Ok(())
        };
        self.message = saved_message(indexed, output);
        self.stage = Stage::Success;
        self.search.clear();
        self.selected_wallet = Some(self.name.clone());
        self.cli_result.set(Some(output));
        Ok(())
    }
    fn paste(&mut self) -> Result<()> {
        if self.stage != Stage::Confirm {
            return Err(Failure::Input);
        }
        let text = self.clipboard.paste_confirmation()?;
        self.backup_confirmation.clear();
        self.backup_confirmation.push_str(&text);
        Ok(())
    }
    fn title(&self) -> &'static str {
        match self.stage {
            Stage::Welcome => "Add wallet",
            Stage::Wallets => "Saved wallets",
            Stage::Details => {
                if self.creating {
                    "New wallet"
                } else {
                    "Import wallet"
                }
            }
            Stage::Secrets => {
                if self.creating {
                    "Create a new wallet"
                } else {
                    "Import an existing wallet"
                }
            }
            Stage::Reveal => "Secure your wallet",
            Stage::Backup => "Back up your recovery phrase",
            Stage::Confirm => "Confirm your recovery phrase",
            Stage::ViewPhrase => "Recovery phrase",
            Stage::Success => {
                if self.creating {
                    "Wallet created"
                } else {
                    "Wallet imported"
                }
            }
            Stage::Delete => "Confirm deletion",
            Stage::Fatal => "Unable to open the wallet",
        }
    }
    fn content(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, focused: bool) {
        match self.stage {
            Stage::Welcome => {
                let mut choice = None;
                ui.vertical_centered(|ui| {
                    ui.add_space(12.0);
                    theme::app_icon(ui);
                    ui.add_space(14.0);
                    ui.label(egui::RichText::new("Add wallet").size(23.0).strong());
                    ui.add_space(24.0);
                    if theme::launch(ui, "Create a new wallet", true).clicked() {
                        choice = Some(true);
                    }
                    if theme::launch(ui, "Import an existing wallet", false).clicked() {
                        choice = Some(false);
                    }
                    if theme::launch(ui, "Cancel", false).clicked() {
                        self.show_wallets();
                    }
                    ui.add_space(12.0);
                });
                if let Some(creating) = choice {
                    self.creating = creating;
                    self.no_passphrase = false;
                    self.copied = false;
                    self.name.clear();
                    self.expected.clear();
                    self.curve = Curve::Secp256k1;
                    self.path = DEFAULT_ETH_PATH.into();
                    self.message.clear();
                    self.stage = Stage::Details;
                }
            }
            Stage::Wallets => self.wallet_browser(ui),
            Stage::Details => {
                let previous = self.curve;
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.curve, Curve::Secp256k1, "Ethereum")
                        .on_hover_text("secp256k1");
                    ui.selectable_value(&mut self.curve, Curve::Ed25519, "Solana")
                        .on_hover_text("Ed25519 · SLIP-0010");
                });
                ui.add_space(8.0);
                if self.curve != previous {
                    self.expected.clear();
                    self.path = if self.curve == Curve::Secp256k1 {
                        DEFAULT_ETH_PATH.into()
                    } else {
                        DEFAULT_SOL_PATH.into()
                    };
                }
                theme::public_field(ui, "Wallet name", &mut self.name, 64);
                theme::public_field(ui, "Derivation path", &mut self.path, 128);
                if self.curve == Curve::Ed25519 {
                    theme::hint(ui, "Hardened path only. Use the original wallet's path when importing.");
                }
                if !self.creating {
                    theme::public_field(
                        ui,
                        if self.curve == Curve::Secp256k1 {
                            "Expected Ethereum address"
                        } else {
                            "Expected Ed25519 public key (0x hex, 32 bytes)"
                        },
                        &mut self.expected,
                        66,
                    );
                    theme::hint(ui,"Use a value from your existing wallet or trusted backup to verify the restored account.");
                }
                let (next, back) = theme::actions(ui, "Back", "Continue");
                if next {
                    let result = if self.creating {
                        self.request().validate()
                    } else {
                        self.request().validate_identifier(&self.expected)
                    };
                    match result {
                        Ok(()) => {
                            self.message.clear();
                            self.stage = Stage::Secrets;
                        }
                        Err(e) => self.message = e.message().into(),
                    }
                }
                if back {
                    self.back();
                }
            }
            Stage::Secrets => {
                theme::hint(
                    ui,
                    &format!("{}  /  {}  /  {}", self.name, self.curve.name(), self.path),
                );
                theme::note(ui,"This optional passphrase changes the wallet you derive. It is not your Mac or wallet unlock password. Spaces are preserved.");
                if focused {
                    if !self.creating {
                        secret_field(
                            ui,
                            "English secret recovery phrase",
                            "mnemonic",
                            &mut self.mnemonic,
                        );
                    }
                    secret_field(
                        ui,
                        "BIP-39 passphrase (optional)",
                        "passphrase",
                        &mut self.passphrase,
                    );
                    secret_field(
                        ui,
                        "Confirm BIP-39 passphrase",
                        "repeat-passphrase",
                        &mut self.repeated_passphrase,
                    );
                    ui.checkbox(&mut self.no_passphrase, "I use no BIP-39 passphrase");
                    theme::hint(ui, "Clipboard actions are disabled on this page.");
                    ui.add_space(8.0);
                } else {
                    ui.label("Focus this window to enter wallet secrets.");
                }
                let (next, cancel) = theme::actions(
                    ui,
                    "Cancel",
                    if self.creating {
                        "Continue to backup"
                    } else {
                        "Verify and save"
                    },
                );
                if next && focused {
                    if let Err(error) = self.prepare() {
                        self.message = error.message().into();
                    }
                }
                if cancel {
                    if self.cli_import {
                        self.cli_result.set(Some(Err(Failure::Cancelled)));
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                    self.back();
                }
            }
            Stage::Reveal => {
                theme::note(ui,"Anyone with this phrase can access your funds. This app cannot recover it for you.");
                ui.label("Write all 12 words down in order and keep them offline. Reveal the phrase only when no one else can see your screen.");
                ui.add_space(14.0);
                let (reveal, cancel) = theme::actions(ui, "Cancel", "Reveal recovery phrase");
                if reveal {
                    self.message.clear();
                    self.stage = Stage::Backup;
                }
                if cancel {
                    self.back();
                }
            }
            Stage::Backup => {
                if focused {
                    if let Some(pending) = &self.pending {
                        recovery_words(ui, pending.phrase());
                        ui.add_space(16.0);
                        if theme::secondary(
                            ui,
                            if self.copied {
                                "Copied"
                            } else {
                                "Copy recovery phrase"
                            },
                        )
                        .clicked()
                        {
                            match self.clipboard.copy(pending.phrase()) {
                                Ok(()) => self.copied = true,
                                Err(e) => self.message = e.message().into(),
                            }
                        }
                    }
                    theme::hint(ui,"Other apps may keep clipboard copies. This app clears its copy after 60 seconds while running, unless you copy something else.");
                } else {
                    ui.label("Recovery phrase hidden while this window is inactive.");
                }
                let (next, cancel) =
                    theme::actions(ui, "Cancel", "I have saved my recovery phrase");
                if next && focused {
                    self.message.clear();
                    self.stage = Stage::Confirm;
                }
                if cancel {
                    self.back();
                }
            }
            Stage::Confirm => {
                ui.label("Enter all 12 words in order to check your backup. Your wallet is saved only after they match.");
                if focused {
                    let response = secret_field(
                        ui,
                        "Secret recovery phrase",
                        "backup-confirmation",
                        &mut self.backup_confirmation,
                    );
                    self.gate
                        .allow_paste
                        .store(response.has_focus(), Ordering::SeqCst);
                    if theme::secondary(ui, "Paste recovery phrase").clicked() {
                        if let Err(e) = self.paste() {
                            self.message = e.message().into();
                        }
                        response.request_focus();
                    }
                }
                let (save, cancel) = theme::actions(ui, "Cancel", "Confirm and save");
                if save && focused {
                    let result = self
                        .pending
                        .as_mut()
                        .ok_or(Failure::Input)
                        .and_then(|p| p.confirm(&self.backup_confirmation))
                        .and_then(|_| self.save());
                    if let Err(e) = result {
                        self.message = if e == Failure::Input {
                            "Recovery phrase does not match. Check all 12 words in order. Nothing was saved.".into()
                        } else {
                            e.message().into()
                        };
                    }
                }
                if cancel {
                    self.back();
                }
            }
            Stage::Success => {
                theme::hint(
                    ui,
                    &format!("{}  /  {}  /  {}", self.name, self.curve.name(), self.path),
                );
                theme::label(ui, "Public key");
                ui.label(egui::RichText::new(&self.public_key).monospace());
                if self.curve == Curve::Secp256k1 {
                    theme::label(ui, "Address");
                    ui.label(egui::RichText::new(&self.identifier).monospace());
                }
                if self.cli_import {
                    if theme::primary(ui, "Done").clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                } else if theme::primary(ui, "Back to wallets").clicked() {
                    self.back();
                }
            }
            Stage::ViewPhrase => {
                theme::label(ui, &self.name);
                ui.add_space(16.0);
                self.recovery_card(ui, &self.name.clone());
                ui.add_space(16.0);
                let shown = self.revealed.visible(Instant::now(), focused).is_some();
                let (back, copy, hide) = theme::recovery_actions(ui, shown, self.copied);
                if copy {
                    if let Some(phrase) = self.revealed.visible(Instant::now(), focused) {
                        match self.clipboard.copy(phrase) {
                            Ok(()) => self.copied = true,
                            Err(_) => self.message = "Unable to copy the recovery phrase.".into(),
                        }
                    }
                }
                if hide {
                    self.revealed.clear();
                    ctx.request_repaint();
                }
                if back {
                    self.back();
                }
                if self.clipboard.has_copy() {
                    ui.add_space(16.0);
                    egui::Frame::new().fill(theme::WARNING_SOFT).corner_radius(8).inner_margin(12)
                        .show(ui, |ui| {
                            ui.colored_label(theme::WARNING_INK, "Recovery phrase is on your clipboard.");
                            ui.label("Other apps may read or keep a copy. It will clear after 60 seconds while this app is running.");
                            ui.add_space(6.0);
                            if theme::secondary(ui, "Clear clipboard").clicked() {
                                self.clipboard.clear();
                                self.copied = false;
                            }
                        });
                }
            }
            Stage::Delete => {
                theme::note(ui,"Deletion is permanent. Without an independent recovery backup, these wallets cannot be recovered.");
                if let Some(plan) = &mut self.deletion {
                    ui.label(format!(
                        "{} wallet(s) to delete; {} already deleted in this operation.",
                        plan.remaining.len(),
                        plan.deleted
                    ));
                    for name in &plan.remaining {
                        ui.label(egui::RichText::new(name).monospace());
                    }
                    theme::hint(ui,"macOS authentication is required for each wallet. A cancellation or failure stops the remaining deletions; completed deletions cannot be undone.");
                    ui.checkbox(
                        &mut plan.confirmed,
                        "I have an offline backup and understand deletion is permanent.",
                    );
                }
                ui.separator();
                let mut cancel = false;
                let mut delete = false;
                ui.horizontal(|ui| {
                    cancel = theme::secondary(ui, "Cancel").clicked();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let ready = focused && self.deletion.as_ref().is_some_and(|p| p.confirmed);
                        delete = ui
                            .add_enabled_ui(ready, |ui| {
                                theme::danger(ui, "Authenticate and delete")
                            })
                            .inner
                            .clicked();
                    });
                });
                if cancel {
                    if self.cli_delete {
                        self.cli_result.set(Some(Err(Failure::Cancelled)));
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    } else {
                        self.return_to_wallets();
                    }
                }
                if delete && focused {
                    self.prepare_authentication();
                    let vault = &self.vault;
                    let wallets = &mut self.wallets;
                    let mut index_failed = false;
                    let result = self
                        .deletion
                        .as_mut()
                        .ok_or(Failure::Input)
                        .and_then(|plan| {
                            plan.execute(|name| {
                                vault.delete_authenticated(name)?;
                                wallets.retain(|entry| entry.name != name);
                                if Catalog::for_group(&vault.group)
                                    .and_then(|c| c.remove(name))
                                    .is_err()
                                {
                                    index_failed = true;
                                }
                                Ok(())
                            })
                        });
                    match result {
                            Ok(()) if self.cli_delete => {
                                match crate::emit(&serde_json::json!({"name":self.name})) {
                                    Ok(()) => { self.cli_result.set(Some(Ok(()))); ctx.send_viewport_cmd(ViewportCommand::Close); }
                                    Err(error) => { self.cli_result.set(Some(Err(error))); ctx.send_viewport_cmd(ViewportCommand::Close); }
                                }
                            }
                            Ok(()) => self.return_to_wallets(),
                            Err(error) => self.message = format!("Deletion stopped ({}). The counts above show what was deleted and what remains. Review the remaining names before confirming again.",error.code()),
                        }
                    if index_failed {
                        self.message.push_str(" A wallet was deleted from Keychain, but the local name list could not be updated. Use Sync from Keychain to repair it.");
                    }
                }
            }
            Stage::Fatal => {
                ui.label(&self.message);
                if theme::secondary(ui, "Quit").clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
        }
        if !matches!(self.stage, Stage::Fatal | Stage::Wallets) && !self.message.is_empty() {
            ui.add_space(12.0);
            egui::Frame::new()
                .fill(theme::DANGER_SOFT)
                .corner_radius(8)
                .inner_margin(12)
                .show(ui, |ui| {
                    ui.colored_label(theme::DANGER, &self.message);
                });
        }
    }
}

impl eframe::App for WalletApp {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        filter_input(raw);
    }
    fn persist_egui_memory(&self) -> bool {
        false
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let focused = ctx.input(|i| i.focused);
        if self.stage != Stage::ViewPhrase {
            self.revealed.clear();
        }
        let phrase_visible = self.revealed.visible(Instant::now(), focused).is_some();
        let contains_secrets = phrase_visible || matches!(
            self.stage,
            Stage::Secrets | Stage::Reveal | Stage::Backup | Stage::Confirm
        );
        if let Err(error) = self.secure_input.set(focused && contains_secrets) {
            self.wipe_draft();
            self.cli_result.set(Some(Err(error)));
            self.message = error.message().into();
            self.stage = Stage::Fatal;
        }
        self.clipboard.tick(Instant::now());
        if !self.clipboard.has_copy() {
            self.copied = false;
        }
        ctx.request_repaint_after(Duration::from_millis(250));
        if self.gate.paste_requested.swap(false, Ordering::SeqCst)
            && self.stage == Stage::Confirm
            && focused
        {
            if let Err(error) = self.paste() {
                self.message = error.message().into();
            }
        }
        self.gate.allow_paste.store(false, Ordering::SeqCst);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::CANVAS)
                    .inner_margin(egui::Margin::same(if self.stage == Stage::Wallets {
                        0
                    } else {
                        28
                    })),
            )
            .show(ctx, |ui| {
                if self.stage == Stage::Wallets {
                    self.content(ui, ctx, focused);
                    return;
                }
                egui::ScrollArea::vertical()
                    .id_salt("page-scroll")
                    .show(ui, |ui| {
                        let width = ui.available_width().min(500.0);
                        ui.vertical_centered(|ui| {
                            ui.set_width(width);
                            ui.add_space(20.0);
                            egui::Frame::new()
                                .fill(egui::Color32::WHITE)
                                .corner_radius(12)
                                .inner_margin(24)
                                .show(ui, |ui| {
                                    ui.set_width((width - 48.0).max(100.0));
                                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                                        if self.stage != Stage::Welcome {
                                            ui.label(egui::RichText::new(self.title()).size(21.0).strong());
                                            ui.add_space(20.0);
                                        }
                                        self.content(ui, ctx, focused);
                                    });
                                });
                        });
                    });
            });
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.wipe_draft();
        self.clipboard.clear();
        let _ = self.secure_input.set(false);
    }
}

pub fn run(
    vault: Vault,
    import: Option<(KeyRequest, String)>,
    delete: Option<String>,
) -> Result<()> {
    let gate = ClipboardGate::new()?;
    let cli_mode = import.is_some() || delete.is_some();
    let outcome = Rc::new(Cell::new(None));
    let mut app = WalletApp {
        vault,
        stage: Stage::Welcome,
        creating: false,
        name: String::new(),
        path: DEFAULT_ETH_PATH.into(),
        curve: Curve::Secp256k1,
        expected: String::new(),
        mnemonic: Zeroizing::new(String::new()),
        passphrase: Zeroizing::new(String::new()),
        repeated_passphrase: Zeroizing::new(String::new()),
        backup_confirmation: Zeroizing::new(String::new()),
        no_passphrase: false,
        pending: None,
        revealed: PhraseReveal::default(),
        recovery_card_height: 196.0,
        public_key: String::new(),
        identifier: String::new(),
        message: String::new(),
        copied: false,
        clipboard: Clipboard::new(),
        gate,
        secure_input: SecureInput::new(),
        cli_import: import.is_some(),
        cli_delete: delete.is_some(),
        wallets: Vec::new(),
        search: String::new(),
        selected_wallet: None,
        show_wallet_details: false,
        deletion: None,
        cli_result: outcome.clone(),
    };
    if let Some((request, expected)) = import {
        app.name = request.name;
        app.path = request.path;
        app.curve = request.curve;
        app.expected = expected;
        app.stage = Stage::Secrets;
    }
    if let Some(name) = delete {
        app.name = name.clone();
        app.confirm_delete(vec![name]);
    }
    if !cli_mode {
        app.show_wallets();
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 640.0])
            .with_min_inner_size([560.0, 560.0])
            .with_drag_and_drop(false),
        ..Default::default()
    };
    eframe::run_native(
        "keychain-wallet",
        options,
        Box::new(move |cc| {
            theme::configure(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
    .map_err(|_| Failure::Configuration)?;
    if cli_mode {
        outcome.take().unwrap_or(Err(Failure::Cancelled))
    } else {
        outcome.take().unwrap_or(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_wallet_reports_delivery_failure_without_claiming_save_failed() {
        assert!(saved_message(true, Ok(())).is_empty());
        let message = saved_message(true, Err(Failure::Transport));
        assert!(message.contains("Wallet saved in Keychain"));
        assert!(message.contains("Do not import it again"));
        assert!(!message.contains("Nothing was saved"));
        let message = saved_message(false, Err(Failure::Transport));
        assert!(message.contains("Sync from Keychain"));
        assert!(message.contains("could not be delivered"));
    }
    #[test]
    fn hiding_phrase_keeps_card_and_footer_in_place() {
        for width in [360.0, 452.0] {
            for count in [12, 24] {
                let ctx = egui::Context::default();
                theme::configure(&ctx);
                let mut height = 196.0;
                let mut footer_top: Option<f32> = None;
                for revealed in [true, false, true] {
                    let _ = ctx.run(egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO, egui::vec2(width + 32.0, 900.0),
                        )),
                        ..Default::default()
                    }, |ctx| {
                        egui::CentralPanel::default().frame(egui::Frame::new().inner_margin(16))
                            .show(ctx, |ui| {
                                let bounds = ui.max_rect();
                                let card = if revealed {
                                    revealed_card(ui, &"abstract ".repeat(count), height)
                                } else {
                                    theme::recovery_mask(ui, height)
                                };
                                assert!(bounds.contains_rect(card.rect), "width={width} count={count} revealed={revealed} bounds={bounds:?} card={:?}", card.rect);
                                height = card.rect.height();
                                ui.add_space(16.0);
                                let footer = ui.scope(|ui| theme::recovery_actions(ui, revealed, false));
                                if let Some(top) = footer_top {
                                    assert!((footer.response.rect.top() - top).abs() < 0.5);
                                }
                                footer_top = Some(footer.response.rect.top());
                            });
                    });
                }
            }
        }
    }

    #[test]
    fn revealed_phrase_expires_and_cannot_reappear_after_focus_loss() {
        let now = Instant::now();
        let mut reveal = PhraseReveal::default();
        assert!(reveal.visible(now, true).is_none());
        reveal.show("public display fixture".into(), now);
        assert_eq!(reveal.visible(now + Duration::from_secs(59), true), Some("public display fixture"));
        assert!(reveal.visible(now + Duration::from_secs(60), true).is_none());
        assert!(reveal.phrase.is_empty());
        assert!(reveal.deadline.is_none());

        reveal.show("another public fixture".into(), now);
        assert!(reveal.visible(now, false).is_none());
        assert!(reveal.phrase.is_empty());
        assert!(reveal.visible(now, true).is_none());

        reveal.show("public display fixture".into(), now);
        reveal.clear();
        assert!(reveal.phrase.is_empty());
        assert!(reveal.visible(now, true).is_none());
    }

    #[test]
    fn recovery_grid_fits_twelve_and_twenty_four_words() {
        for width in [360.0, 452.0] {
            for count in [12, 24] {
                let ctx = egui::Context::default();
                theme::configure(&ctx);
                let raw = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO, egui::vec2(width + 32.0, 600.0),
                    )),
                    ..Default::default()
                };
                let _ = ctx.run(raw, |ctx| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::new().inner_margin(16))
                        .show(ctx, |ui| {
                            let bounds = ui.max_rect();
                            let grid = ui.scope(|ui| recovery_words(ui, &"abstract ".repeat(count)));
                            assert!(bounds.contains_rect(grid.response.rect), "width={width}, count={count}, bounds={bounds:?}, grid={:?}", grid.response.rect);
                        });
                });
            }
        }
    }

    #[test]
    fn clipboard_and_file_input_cannot_bypass_explicit_confirmation_paste() {
        let mut raw = egui::RawInput {
            events: vec![
                Event::Copy,
                Event::Cut,
                Event::Paste("public sentinel".into()),
                Event::Text("t".into()),
            ],
            ..Default::default()
        };
        filter_input(&mut raw);
        assert_eq!(raw.events, vec![Event::Text("t".into())]);
    }
}
