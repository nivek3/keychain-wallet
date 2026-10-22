use bip32::{DerivationPath, XPrv};
use bip39::{Language, Mnemonic};
use ed25519_dalek_bip32::{DerivationPath as EdPath, ExtendedSigningKey};
use sha3::{Digest, Keccak256};
use std::{slice, str};
use zeroize::{Zeroize, Zeroizing};

/// Safety: buffer must be writable for len bytes, or null with len zero.
#[no_mangle]
pub unsafe extern "C" fn mw_zeroize(buffer: *mut u8, len: usize) {
    if !buffer.is_null() {
        slice::from_raw_parts_mut(buffer, len).zeroize();
    }
}

/// Safety: entropy must contain 16 bytes; output must be writable for 256 bytes.
/// Buffers must not overlap. The UTF-8 output is zero-terminated, including on error.
#[no_mangle]
pub unsafe extern "C" fn mw_mnemonic_from_entropy(entropy: *const u8, output: *mut u8) -> i32 {
    if output.is_null() {
        return 4;
    }
    std::ptr::write_bytes(output, 0, 256);
    if entropy.is_null() {
        return 4;
    }
    let mnemonic =
        match Mnemonic::from_entropy_in(Language::English, slice::from_raw_parts(entropy, 16)) {
            Ok(value) => value,
            Err(_) => return 1,
        };
    let words = Zeroizing::new(mnemonic.to_string());
    if words.len() >= 256 {
        return 4;
    }
    std::ptr::copy_nonoverlapping(words.as_ptr(), output, words.len());
    0
}

fn derive_seed(seed: &[u8], curve: u32, path: &str) -> Result<(Zeroizing<[u8; 32]>, Vec<u8>), i32> {
    match curve {
        0 => {
            let path: DerivationPath = path.parse().map_err(|_| 2)?;
            let child = XPrv::derive_from_path(seed, &path).map_err(|_| 3)?;
            Ok((Zeroizing::new(child.to_bytes()), child.private_key().verifying_key().to_encoded_point(false).as_bytes().to_vec()))
        }
        1 => {
            let path: EdPath = path.parse().map_err(|_| 2)?;
            // This backend implements SLIP-0010 and rejects non-hardened children.
            let root = ExtendedSigningKey::from_seed(seed).map_err(|_| 3)?;
            let child = root.derive(&path).map_err(|_| 3)?;
            Ok((Zeroizing::new(child.signing_key.to_bytes()), child.verifying_key().to_bytes().to_vec()))
        }
        _ => Err(4),
    }
}

pub fn derive(mnemonic: &str, passphrase: &str, curve: u32, path: &str)
    -> Result<(Zeroizing<[u8; 32]>, Vec<u8>), i32> {
    let mnemonic = Mnemonic::parse_in(Language::English, mnemonic).map_err(|_| 1)?;
    let seed = Zeroizing::new(mnemonic.to_seed(passphrase));
    derive_seed(&seed[..], curve, path)
}

pub fn ethereum_address(public: &[u8]) -> [u8; 20] {
    let hash = Keccak256::digest(&public[1..]);
    let mut address = [0; 20];
    address.copy_from_slice(&hash[12..]);
    address
}

/// Safety: public must contain 65 bytes, address_out must be writable for 20 bytes.
#[no_mangle]
pub unsafe extern "C" fn mw_ethereum_address(public: *const u8, address_out: *mut u8) -> i32 {
    if address_out.is_null() { return 4; }
    std::ptr::write_bytes(address_out, 0, 20);
    if public.is_null() || *public != 4 { return 4; }
    let address = ethereum_address(slice::from_raw_parts(public, 65));
    std::ptr::copy_nonoverlapping(address.as_ptr(), address_out, 20);
    0
}

/// Legacy C ABI; the desktop app calls the safe Rust API. No secret is logged across this ABI.
/// Safety: inputs must be valid nonoverlapping buffers of the stated lengths;
/// outputs must be writable buffers of 32 and 65 bytes respectively.
/// Curve 0 = secp256k1, curve 1 = SLIP-0010 Ed25519 (public output uses its first 32 bytes).
#[no_mangle]
pub unsafe extern "C" fn mw_derive(
    curve: u32,
    mnemonic: *const u8,
    mnemonic_len: usize,
    passphrase: *const u8,
    passphrase_len: usize,
    path: *const u8,
    path_len: usize,
    key_out: *mut u8,
    public_out: *mut u8,
) -> i32 {
    if key_out.is_null() || public_out.is_null() { return 4; }
    std::ptr::write_bytes(key_out, 0, 32);
    std::ptr::write_bytes(public_out, 0, 65);
    if mnemonic.is_null()
        || passphrase.is_null()
        || path.is_null()
        || key_out.is_null()
        || public_out.is_null()
        || mnemonic_len > 4096
        || passphrase_len > 4096
        || path_len > 128
    {
        return 4;
    }
    let inputs = (
        str::from_utf8(slice::from_raw_parts(mnemonic, mnemonic_len)),
        str::from_utf8(slice::from_raw_parts(passphrase, passphrase_len)),
        str::from_utf8(slice::from_raw_parts(path, path_len)),
    );
    match inputs {
        (Ok(m), Ok(p), Ok(d)) => match derive(m, p, curve, d) {
            Ok((key, public)) => {
                std::ptr::copy_nonoverlapping(key.as_ptr(), key_out, 32);
                std::ptr::copy_nonoverlapping(public.as_ptr(), public_out, public.len());
                0
            }
            Err(code) => code,
        },
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const WORDS: &str = "test test test test test test test test test test test junk";
    const PATH: &str = "m/44'/60'/0'/0/0";

    fn bytes(hex: &str) -> Vec<u8> {
        (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn slip10_official_ed25519_vector() {
        // SLIP-0010 test vector 1: master and final hardened child.
        let seed = bytes("000102030405060708090a0b0c0d0e0f");
        for (path, secret, public) in [
            ("m", "2b4be7f19ee27bbf30c667b642d5f4aa69fd169872f8fc3059c08ebae2eb19e7", "a4b2856bfec510abab89753fac1ac0e1112364e7d250545963f135f2a33188ed"),
            ("m/0'/1'/2'/2'/1000000000'", "8f94d394a8e8fd6b1bc2f3f49f5c47e385281d5c17e65324b0f62483e37e8793", "3c24da049451555d51a7014a37337aa4e12d41e485abccfa46b47dfb2af54b7a"),
        ] {
            let (key, result) = derive_seed(&seed, 1, path).unwrap();
            assert_eq!(key.as_slice(), bytes(secret));
            assert_eq!(result, bytes(public));
        }
        assert!(derive_seed(&seed, 1, "m/0'/1").is_err());
        assert!(derive_seed(&seed, 2, "m/0'").is_err());
    }

    #[test]
    fn ffi_keypair_lengths_and_error_clearing() {
        let mut key = [0xa5; 32];
        let mut public = [0xa5; 65];
        let path = "m/44'/501'/0'/0'";
        for (curve, path, expected) in [(1, path, 0), (1, "m/0'/1", 3), (5, path, 4)] {
            let code = unsafe { mw_derive(curve, WORDS.as_ptr(), WORDS.len(), b"".as_ptr(), 0,
                path.as_ptr(), path.len(), key.as_mut_ptr(), public.as_mut_ptr()) };
            assert_eq!(code, expected);
            if code == 0 {
                assert!(public[..32].iter().any(|b| *b != 0));
                assert_eq!(&public[32..], &[0; 33]);
                let (reference_key, reference_public) = derive(WORDS, "", 1, path).unwrap();
                assert_eq!(key, *reference_key);
                assert_eq!(&public[..32], reference_public);
            } else {
                assert_eq!(key, [0; 32]);
                assert_eq!(public, [0; 65]);
            }
        }
        key.fill(0xa5);
        public.fill(0xa5);
        assert_eq!(unsafe { mw_derive(0, std::ptr::null(), 0, b"".as_ptr(), 0,
            PATH.as_ptr(), PATH.len(), key.as_mut_ptr(), public.as_mut_ptr()) }, 4);
        assert_eq!(key, [0; 32]);
        assert_eq!(public, [0; 65]);
    }

    #[test]
    fn entropy_to_mnemonic_bip39_vector_and_error_clearing() {
        let entropy = [0u8; 16];
        let mut output = [0xa5u8; 256];
        assert_eq!(
            unsafe { mw_mnemonic_from_entropy(entropy.as_ptr(), output.as_mut_ptr()) },
            0
        );
        let len = output.iter().position(|b| *b == 0).unwrap();
        let words = str::from_utf8(&output[..len]).unwrap();
        assert_eq!(words, "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        assert_eq!(
            Mnemonic::parse_in(Language::English, words)
                .unwrap()
                .to_entropy(),
            entropy
        );
        assert_eq!(
            unsafe { mw_mnemonic_from_entropy(std::ptr::null(), output.as_mut_ptr()) },
            4
        );
        assert_eq!(output, [0; 256]);
    }

    #[test]
    fn ffi_wipe_clears_caller_buffer() {
        let mut buffer = [0xa5; 64];
        unsafe {
            mw_zeroize(buffer.as_mut_ptr(), buffer.len());
        }
        assert_eq!(buffer, [0; 64]);
        unsafe {
            mw_zeroize(std::ptr::null_mut(), 0);
        }
    }

    #[test]
    fn public_hardhat_vector() {
        let (_, public) = derive(WORDS, "", 0, PATH).unwrap();
        let address = ethereum_address(&public);
        assert_eq!(
            address,
            [
                0xf3, 0x9f, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xf6, 0xf4, 0xce, 0x6a, 0xb8, 0x82, 0x72,
                0x79, 0xcf, 0xff, 0xb9, 0x22, 0x66
            ]
        );
    }

    #[test]
    fn bip39_passphrase_vector_and_nfkd() {
        let words = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic = Mnemonic::parse_in(Language::English, words).unwrap();
        let seed = Zeroizing::new(mnemonic.to_seed("TREZOR"));
        let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e53495531f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04");
        assert_eq!(
            *derive(WORDS, "é", 0, PATH).unwrap().0,
            *derive(WORDS, "e\u{301}", 0, PATH).unwrap().0
        );
        assert_ne!(
            *derive(WORDS, "", 0, PATH).unwrap().0,
            *derive(WORDS, " ", 0, PATH).unwrap().0
        );
    }

    #[test]
    fn rejects_bad_checksum_and_path() {
        assert!(derive("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon", "", 0, PATH).is_err());
        assert!(derive(WORDS, "", 0, "bad/path").is_err());
    }
}

/// Convert OS-supplied entropy to an English BIP-39 phrase without crossing an FFI boundary.
pub fn mnemonic_from_entropy(entropy: &[u8; 16]) -> Result<Zeroizing<String>, i32> {
    Mnemonic::from_entropy_in(Language::English, entropy).map(|m| Zeroizing::new(m.to_string())).map_err(|_| 1)
}
