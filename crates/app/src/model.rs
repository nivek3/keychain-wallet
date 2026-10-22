use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

pub type Result<T> = std::result::Result<T, Failure>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Cancelled, Authentication, Missing, Duplicate, Keychain, Configuration,
    Input, Mnemonic, Derivation, PublicKey, Address, Transport, Corrupt,
}
impl Failure {
    pub fn code(self) -> &'static str {
        match self {
            Self::Cancelled => "CANCELLED", Self::Authentication => "AUTHENTICATION_FAILED",
            Self::Missing => "NOT_FOUND", Self::Duplicate => "ALREADY_EXISTS", Self::Keychain => "KEYCHAIN_ERROR",
            Self::Configuration => "SIGNING_CONFIGURATION", Self::Input => "INVALID_INPUT",
            Self::Mnemonic => "INVALID_MNEMONIC", Self::Derivation => "DERIVATION_FAILED",
            Self::PublicKey => "PUBLIC_KEY_MISMATCH", Self::Address => "ADDRESS_MISMATCH",
            Self::Transport => "TRANSPORT_ERROR", Self::Corrupt => "INVALID_ITEM",
        }
    }
    pub fn message(self) -> &'static str {
        match self {
            Self::Mnemonic => "Check the English recovery words, their order, and checksum.",
            Self::Address => "The phrase, passphrase, and path do not match the expected Ethereum address.",
            Self::PublicKey => "The phrase, passphrase, and path do not match the expected Ed25519 public key.",
            Self::Duplicate => "That name already exists. The existing wallet was not changed.",
            Self::Configuration => "The app's signing or secure-input configuration is invalid. Rebuild with your Personal Team.",
            Self::Authentication => "macOS authentication was not completed. Nothing was unlocked.",
            _ => "Check the wallet details, derivation path, and matching passphrase fields. Nothing was saved.",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Curve {
    #[serde(rename = "secp256k1")] Secp256k1,
    #[serde(rename = "ed25519")] Ed25519,
}
impl Curve {
    pub fn name(self) -> &'static str { match self { Self::Secp256k1 => "secp256k1", Self::Ed25519 => "ed25519" } }
    pub fn id(self) -> u32 { match self { Self::Secp256k1 => 0, Self::Ed25519 => 1 } }
}

#[derive(Serialize, Deserialize)]
pub struct KeyRequest { pub name: String, pub path: String, pub curve: Curve }
impl KeyRequest {
    pub fn validate(&self) -> Result<()> {
        validate_name(&self.name)?;
        if self.path.len() > 128 { return Err(Failure::Input); }
        let parts: Vec<_> = self.path.split('/').collect();
        let number = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
            && s.parse::<u32>().is_ok_and(|n| n < 0x80000000);
        let hardened = |s: &str| s.strip_suffix('\'').is_some_and(number);
        let valid = match self.curve {
            Curve::Secp256k1 => parts.len() == 6 && parts[..3] == ["m", "44'", "60'"]
                && hardened(parts[3]) && matches!(parts[4], "0" | "1") && number(parts[5]),
            Curve::Ed25519 => parts.len() >= 2 && parts[0] == "m" && parts[1..].iter().all(|s| hardened(s)),
        };
        if valid { Ok(()) } else { Err(Failure::Input) }
    }
    pub fn validate_identifier(&self, expected: &str) -> Result<()> {
        self.validate()?;
        let len = if self.curve == Curve::Secp256k1 { 40 } else { 64 };
        if expected.len() != len + 2 || !expected.starts_with("0x")
            || !expected[2..].bytes().all(|b| b.is_ascii_hexdigit()) { return Err(Failure::Input); }
        Ok(())
    }
}
pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 64 || !name.as_bytes()[0].is_ascii_alphanumeric()
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) { Err(Failure::Input) } else { Ok(()) }
}

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct StoredWallet {
    #[zeroize(skip)] pub version: u32,
    pub mnemonic: String,
    pub passphrase: String,
}
pub struct Derived {
    pub key: Zeroizing<[u8; 32]>,
    pub public_key: String,
    pub identifier: String,
}
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut result = String::with_capacity(2 + bytes.len() * 2);
    result.push_str("0x");
    for byte in bytes { let _ = write!(result, "{byte:02x}"); }
    result
}
pub fn normalize(text: &str) -> Zeroizing<String> {
    Zeroizing::new(text.split_whitespace().collect::<Vec<_>>().join(" "))
}
pub fn derive(wallet: &StoredWallet, request: &KeyRequest, expected: Option<&str>) -> Result<Derived> {
    request.validate()?;
    if wallet.version != 1 { return Err(Failure::Corrupt); }
    if wallet.mnemonic.is_empty() || wallet.mnemonic.len() > 4096 || wallet.passphrase.len() > 4096 { return Err(Failure::Input); }
    if let Some(expected) = expected { request.validate_identifier(expected)?; }
    let (key, public) = wallet_crypto::derive(&wallet.mnemonic, &wallet.passphrase, request.curve.id(), &request.path)
        .map_err(|code| if code == 1 { Failure::Mnemonic } else { Failure::Derivation })?;
    let public_key = hex(&public);
    let identifier = if request.curve == Curve::Secp256k1 { hex(&wallet_crypto::ethereum_address(&public)) } else { public_key.clone() };
    if expected.is_some_and(|value| !identifier.eq_ignore_ascii_case(value)) {
        return Err(if request.curve == Curve::Secp256k1 { Failure::Address } else { Failure::PublicKey });
    }
    Ok(Derived { key, public_key, identifier })
}
pub fn check_passphrase(passphrase: &str, confirmation: &str, empty: bool) -> Result<()> {
    if passphrase.len() > 4096 || confirmation.len() > 4096 { return Err(Failure::Input); }
    if if empty { passphrase.is_empty() && confirmation.is_empty() } else { !passphrase.is_empty() && passphrase == confirmation } {
        Ok(())
    } else { Err(Failure::Input) }
}

// Creation cannot reach persistence until the complete backup has matched.
pub struct PendingWallet { wallet: StoredWallet, confirmed: bool }
impl PendingWallet {
    pub fn new(wallet: StoredWallet, creating: bool) -> Self { Self { wallet, confirmed: !creating } }
    pub fn phrase(&self) -> &str { &self.wallet.mnemonic }
    pub fn confirm(&mut self, phrase: &str) -> Result<()> {
        self.confirmed = *normalize(phrase) == self.wallet.mnemonic;
        if self.confirmed { Ok(()) } else { Err(Failure::Input) }
    }
    pub fn for_save(&self) -> Result<&StoredWallet> {
        if self.confirmed { Ok(&self.wallet) } else { Err(Failure::Input) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const WORDS: &str = "test test test test test test test test test test test junk";
    fn wallet(passphrase: &str) -> StoredWallet { StoredWallet { version: 1, mnemonic: WORDS.into(), passphrase: passphrase.into() } }
    fn request(curve: Curve, path: &str) -> KeyRequest { KeyRequest { name: "public-test".into(), curve, path: path.into() } }
    #[test]
    fn path_name_and_public_identifier_boundaries() {
        for name in ["", "x\n", "-x", "钱包", &"x".repeat(65)] { assert_eq!(validate_name(name), Err(Failure::Input)); }
        for path in ["m", "m/0'/1", "m/2147483648'", "m/0'\n", "m//0'"] {
            assert_eq!(request(Curve::Ed25519, path).validate(), Err(Failure::Input));
        }
        for path in ["m/44'/60'/0'/2/0", "m/44'/60'/2147483648'/0/0", "m/44'/60'/0'/0/0\n"] {
            assert_eq!(request(Curve::Secp256k1,path).validate(), Err(Failure::Input));
        }
        assert!(serde_json::from_str::<KeyRequest>(r#"{"name":"x","path":"m/0'"}"#).is_err());
        assert!(serde_json::from_str::<KeyRequest>(r#"{"name":"x","path":"m/0'","curve":"p256"}"#).is_err());
        assert!(request(Curve::Ed25519,"m/0'").validate_identifier(&format!("0x{}", "0".repeat(64))).is_ok());
        assert!(request(Curve::Ed25519,"m/0'").validate_identifier(&format!("0x{}\n", "0".repeat(64))).is_err());
    }
    #[test]
    fn legacy_storage_and_independent_public_vectors() {
        let stored: StoredWallet = serde_json::from_str(&format!(r#"{{"version":1,"mnemonic":"{WORDS}","passphrase":""}}"#)).unwrap();
        let pair = derive(&stored, &request(Curve::Secp256k1,"m/44'/60'/0'/0/0"), Some("0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266")).unwrap();
        assert_eq!(pair.key.len(),32);
        for (passphrase,path,public) in [
            ("","m/44'/501'/0'/0'","0bf32b9f0db09672038fea36139b18f98a5f0149ef4ce0332e44b9a77e83c22d"),
            ("","m/44'/501'/1'/0'","9245441f4a752ea6328c6d8d2d2afcb36d131db998a47ca04d6d5d8587d0b012"),
            ("TREZOR","m/44'/501'/0'/0'","0bd89cf16f4f7a40a2839b8ca08832cebec6737bd30cae4af528e59e6c6b0f33"),
            ("é","m/44'/501'/0'/0'","e0d2365114a62d771831bed83e660ef8d88c0b35bf9a166bc98e1f32b8255254"),
            ("e\u{301}","m/44'/501'/0'/0'","e0d2365114a62d771831bed83e660ef8d88c0b35bf9a166bc98e1f32b8255254"),
        ] {
            assert_eq!(derive(&wallet(passphrase), &request(Curve::Ed25519,path),Some(&format!("0x{public}"))).unwrap().public_key, format!("0x{public}"));
        }
        assert!(matches!(derive(&stored,&request(Curve::Ed25519,"m/0'"), Some(&format!("0x{}","0".repeat(64)))),Err(Failure::PublicKey)));
        assert!(matches!(derive(&wallet(" "),&request(Curve::Secp256k1,"m/44'/60'/0'/0/0"),Some(&pair.identifier)),Err(Failure::Address)));
    }
    #[test]
    fn backup_gate_and_explicit_passphrase() {
        assert!(check_passphrase("","",false).is_err());
        assert!(check_passphrase(" "," ",false).is_ok());
        assert!(check_passphrase(" "," ",true).is_err());
        assert!(check_passphrase("","",true).is_ok());
        assert!(check_passphrase("x","y",false).is_err());
        let mut pending = PendingWallet::new(wallet(""),true);
        assert!(pending.for_save().is_err());
        assert!(pending.confirm("wrong backup").is_err());
        assert!(pending.for_save().is_err());
        assert!(pending.confirm(&format!("  {WORDS}\n")).is_ok());
        assert!(pending.for_save().is_ok());
        assert!(pending.confirm("wrong backup").is_err());
        assert!(pending.for_save().is_err());
    }
}
