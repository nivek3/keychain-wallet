use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, RawFd},
        unix::{net::UnixStream, process::CommandExt},
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

#[cfg(not(test))]
mod node;

type BridgeResult<T> = Result<T, &'static str>;
const INVALID: &str = "Invalid wallet selection";
const BAD_RESPONSE: &str = "INVALID_HELPER_RESPONSE";
const EXECUTABLE: &str = "keychain-wallet.app/Contents/MacOS/keychain-wallet";

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum Curve {
    Secp256k1,
    Ed25519,
}

impl Curve {
    fn as_str(self) -> &'static str {
        match self {
            Self::Secp256k1 => "secp256k1",
            Self::Ed25519 => "ed25519",
        }
    }
}

#[derive(Clone, Hash, PartialEq, Eq)]
struct Selection {
    helper: PathBuf,
    name: String,
    curve: Curve,
    path: String,
}

#[derive(Clone)]
struct KeyPair {
    private: Zeroizing<[u8; 32]>,
    public: String,
}

// The lock serializes system authentication. Both successes and failures remain process-local.
static SESSIONS: OnceLock<Mutex<HashMap<Selection, BridgeResult<KeyPair>>>> = OnceLock::new();

fn validate(input: &Value) -> BridgeResult<(String, Curve, String, Option<PathBuf>)> {
    let name = input.get("name").and_then(Value::as_str).ok_or(INVALID)?;
    let curve = match input.get("curve").and_then(Value::as_str) {
        Some("secp256k1") => Curve::Secp256k1,
        Some("ed25519") => Curve::Ed25519,
        _ => return Err(INVALID),
    };
    if name.is_empty()
        || name.len() > 64
        || !name.as_bytes()[0].is_ascii_alphanumeric()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err(INVALID);
    }
    let index = match input.get("index") {
        None => 0,
        Some(value) => {
            let number = value.as_f64().ok_or(INVALID)?;
            if !number.is_finite()
                || number.fract() != 0.0
                || !(0.0..2147483648.0).contains(&number)
            {
                return Err(INVALID);
            }
            number as u32
        }
    };
    if input.get("path").is_some() && input.get("index").is_some() {
        return Err(INVALID);
    }
    let path = match input.get("path") {
        Some(value) => value.as_str().ok_or(INVALID)?.to_owned(),
        None if curve == Curve::Secp256k1 => format!("m/44'/60'/0'/0/{index}"),
        None => return Err(INVALID),
    };
    if path.len() > 128 || (curve == Curve::Ed25519 && input.get("index").is_some()) {
        return Err(INVALID);
    }
    let parts: Vec<_> = path.split('/').collect();
    let number = |text: &str| {
        !text.is_empty()
            && text.bytes().all(|b| b.is_ascii_digit())
            && text.parse::<u32>().is_ok_and(|v| v < 0x80000000)
    };
    let valid = if curve == Curve::Ed25519 {
        parts.len() >= 2
            && parts[0] == "m"
            && parts[1..]
                .iter()
                .all(|p| p.strip_suffix('\'').is_some_and(number))
    } else {
        parts.len() == 6
            && parts[..3] == ["m", "44'", "60'"]
            && parts[3].strip_suffix('\'').is_some_and(number)
            && matches!(parts[4], "0" | "1")
            && number(parts[5])
    };
    if !valid {
        return Err(INVALID);
    }
    let helper = input
        .get("helperPath")
        .map(|value| {
            let path = PathBuf::from(value.as_str().ok_or(INVALID)?);
            if path.is_absolute() {
                Ok(path)
            } else {
                Err(INVALID)
            }
        })
        .transpose()?;
    Ok((name.to_owned(), curve, path, helper))
}

fn development_root(package: &Path) -> Option<&Path> {
    let root = package.parent()?;
    (package.file_name()? == "node"
        && root.join("Cargo.toml").is_file()
        && root.join("scripts/build-native.sh").is_file())
    .then_some(root)
}

fn find_helper(
    explicit: Option<PathBuf>,
    home: Option<PathBuf>,
    root: &Path,
) -> BridgeResult<PathBuf> {
    let selected = match explicit {
        Some(path) => path,
        None => home
            .map(|home| home.join("Applications").join(EXECUTABLE))
            .into_iter()
            .chain([Path::new("/Applications").join(EXECUTABLE)])
            .chain(
                development_root(root).map(|repository| repository.join("build").join(EXECUTABLE)),
            )
            .find(|path| path.exists())
            .ok_or("KEYCHAIN_WALLET_APP_NOT_FOUND")?,
    };
    // An explicit or selected broken helper never falls back to another executable.
    selected.canonicalize().map_err(|_| "HELPER_START_FAILED")
}

fn unlock(selection: Selection) -> BridgeResult<KeyPair> {
    let mut sessions = SESSIONS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "KEYCHAIN_ERROR")?;
    sessions
        .entry(selection.clone())
        .or_insert_with(|| read_key(&selection, Duration::from_secs(120)))
        .clone()
}

fn nonblocking(fd: RawFd) -> BridgeResult<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err("TRANSPORT_ERROR");
    }
    Ok(())
}

fn drain(reader: &mut impl Read, output: &mut Vec<u8>, limit: usize) -> BridgeResult<bool> {
    let mut chunk = Zeroizing::new([0u8; 1024]);
    loop {
        match reader.read(&mut *chunk) {
            Ok(0) => return Ok(true),
            Ok(count) if output.len() + count <= limit => output.extend_from_slice(&chunk[..count]),
            Ok(_) => return Err(BAD_RESPONSE),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(false),
            Err(_) => return Err("TRANSPORT_ERROR"),
        }
    }
}

fn read_key(selection: &Selection, timeout: Duration) -> BridgeResult<KeyPair> {
    let (mut secret, child_secret) = UnixStream::pair().map_err(|_| "TRANSPORT_ERROR")?;
    let fd = child_secret.as_raw_fd();
    let mut command = Command::new(&selection.helper);
    command
        .arg("--bridge")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .env("PATH", "/usr/bin:/bin");
    for name in ["HOME", "USER", "LOGNAME", "TMPDIR"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    // After fork, use only async-signal-safe descriptor operations before exec.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(fd, 3) < 0 || libc::fcntl(3, libc::F_SETFD, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().map_err(|_| "HELPER_START_FAILED")?;
    drop(child_secret);
    let result = (|| {
        let mut stdin = child.stdin.take().ok_or("TRANSPORT_ERROR")?;
        let request = json!({"name": selection.name, "curve": selection.curve.as_str(), "path": selection.path});
        stdin
            .write_all(request.to_string().as_bytes())
            .map_err(|_| "TRANSPORT_ERROR")?;
        drop(stdin);
        let mut stdout = child.stdout.take().ok_or("TRANSPORT_ERROR")?;
        let mut stderr = child.stderr.take().ok_or("TRANSPORT_ERROR")?;
        for fd in [secret.as_raw_fd(), stdout.as_raw_fd(), stderr.as_raw_fd()] {
            nonblocking(fd)?;
        }
        let mut key = Zeroizing::new(Vec::with_capacity(32));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let deadline = Instant::now() + timeout;
        loop {
            let key_done = drain(&mut secret, &mut key, 32)?;
            let out_done = drain(&mut stdout, &mut out, 2048)?;
            let err_done = drain(&mut stderr, &mut err, 1024)?;
            if let Some(status) = child.try_wait().map_err(|_| "TRANSPORT_ERROR")? {
                if key_done && out_done && err_done {
                    if !status.success() {
                        return Err(reported_error(&err));
                    }
                    return parse_response(selection, &key, &out, &err);
                }
            }
            if Instant::now() >= deadline {
                return Err("AUTHENTICATION_TIMEOUT");
            }
            thread::sleep(Duration::from_millis(10));
        }
    })();
    if result.is_err() {
        let _ = child.kill();
    }
    let _ = child.wait();
    result
}

fn reported_error(bytes: &[u8]) -> &'static str {
    let message = std::str::from_utf8(bytes).unwrap_or("").trim();
    [
        "CANCELLED",
        "AUTHENTICATION_FAILED",
        "NOT_FOUND",
        "ALREADY_EXISTS",
        "KEYCHAIN_ERROR",
        "SIGNING_CONFIGURATION",
        "INVALID_INPUT",
        "INVALID_MNEMONIC",
        "DERIVATION_FAILED",
        "ADDRESS_MISMATCH",
        "PUBLIC_KEY_MISMATCH",
        "TRANSPORT_ERROR",
        "INVALID_ITEM",
    ]
    .into_iter()
    .find(|code| *code == message)
    .unwrap_or("HELPER_FAILED")
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(2 + bytes.len() * 2);
    out.push_str("0x");
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn parse_response(
    selection: &Selection,
    key: &[u8],
    out: &[u8],
    err: &[u8],
) -> BridgeResult<KeyPair> {
    let response: Value = serde_json::from_slice(out).map_err(|_| BAD_RESPONSE)?;
    if response.get("name").and_then(Value::as_str) != Some(&selection.name)
        || response.get("curve").and_then(Value::as_str) != Some(selection.curve.as_str())
        || response.get("path").and_then(Value::as_str) != Some(&selection.path)
        || !err.iter().all(u8::is_ascii_whitespace)
    {
        return Err(BAD_RESPONSE);
    }
    let private: Zeroizing<[u8; 32]> = Zeroizing::new(key.try_into().map_err(|_| BAD_RESPONSE)?);
    let public = match selection.curve {
        Curve::Secp256k1 => {
            let signing =
                k256::ecdsa::SigningKey::from_slice(&*private).map_err(|_| BAD_RESPONSE)?;
            signing
                .verifying_key()
                .to_encoded_point(true)
                .as_bytes()
                .to_vec()
        }
        Curve::Ed25519 => solana_ed25519::ed_sigs::SigningKey::from_bytes(&private)
            .verification_key()
            .as_ref()
            .to_vec(),
    };
    let public = hex(&public);
    if response.get("publicKey").and_then(Value::as_str) != Some(&public) {
        return Err(BAD_RESPONSE);
    }
    Ok(KeyPair { private, public })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        sync::atomic::{AtomicUsize, Ordering},
    };

    static COUNT: AtomicUsize = AtomicUsize::new(0);
    fn folder() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "keychain-wallet-bridge-{}-{}",
            std::process::id(),
            COUNT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn curves_defaults_and_index_boundaries() {
        for index in [0, 1, 2, 2147483647] {
            let (_, curve, path, _) =
                validate(&json!({"name":"test", "curve":"secp256k1", "index": index})).unwrap();
            assert_eq!(curve, Curve::Secp256k1);
            assert_eq!(path, format!("m/44'/60'/0'/0/{index}"));
        }
        assert_eq!(
            validate(&json!({"name":"test", "curve":"secp256k1"}))
                .unwrap()
                .2,
            "m/44'/60'/0'/0/0"
        );
        assert!(
            validate(&json!({"name":"test", "curve":"ed25519", "path":"m/2147483647'"})).is_ok()
        );
        assert!(
            validate(&json!({"name":"test", "curve":"secp256k1", "path":"m/44'/60'/1'/1/5"}))
                .is_ok()
        );
    }

    #[test]
    fn invalid_selection_never_discovers_or_launches_a_helper() {
        for input in [
            json!({"name":"test"}),
            json!({"name":"test", "curve":"p256"}),
            json!({"name":"test", "curve":"ed25519"}),
            json!({"name":"test", "curve":"ed25519", "path":"m/0'/1"}),
            json!({"name":"test", "curve":"ed25519", "path":"m/0'", "index":0}),
            json!({"name":"test", "curve":"ed25519", "path":"m/2147483648'"}),
            json!({"name":"test", "curve":"ed25519", "path":"m/0'\n"}),
            json!({"name":"bad\n", "curve":"secp256k1"}),
            json!({"name":"test", "curve":"secp256k1", "helperPath":"relative"}),
            json!({"name":"test", "curve":"secp256k1", "path":null}),
        ] {
            assert!(matches!(validate(&input), Err(INVALID)));
        }
        for index in [
            json!(-1),
            json!(0.5),
            json!(2147483648u64),
            json!("1"),
            Value::Null,
        ] {
            assert!(matches!(
                validate(&json!({"name":"test", "curve":"secp256k1", "index":index})),
                Err(INVALID)
            ));
        }
    }

    #[test]
    fn local_build_discovery_requires_the_node_package_inside_the_repository() {
        let root = folder();
        let package = root.join("node");
        fs::create_dir_all(&package).unwrap();
        assert_eq!(development_root(&package), None);
        fs::write(root.join("Cargo.toml"), "public fixture").unwrap();
        assert_eq!(development_root(&package), None);
        fs::create_dir_all(root.join("scripts")).unwrap();
        fs::write(root.join("scripts/build-native.sh"), "public fixture").unwrap();
        assert_eq!(development_root(&package), Some(root.as_path()));
        assert_eq!(development_root(&root.join("keychain-wallet")), None);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn user_helper_and_explicit_paths_do_not_silently_fall_back() {
        let root = folder();
        let helper = root.join("Applications").join(EXECUTABLE);
        fs::create_dir_all(helper.parent().unwrap()).unwrap();
        fs::write(&helper, "public fixture").unwrap();
        assert_eq!(
            find_helper(None, Some(root.clone()), &root).unwrap(),
            helper.canonicalize().unwrap()
        );
        assert!(matches!(
            find_helper(Some(root.join("missing")), Some(root.clone()), &root),
            Err("HELPER_START_FAILED")
        ));
        let link = root.join("helper-link");
        std::os::unix::fs::symlink(&helper, &link).unwrap();
        assert_eq!(
            find_helper(Some(link), None, &root).unwrap(),
            helper.canonicalize().unwrap()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn keypair_response_rejects_metadata_and_key_mismatches() {
        // RFC 8032 public test vector, never use for funds.
        let key_hex = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
        let key: Vec<u8> = (0..64)
            .step_by(2)
            .map(|i| u8::from_str_radix(&key_hex[i..i + 2], 16).unwrap())
            .collect();
        let selection = Selection {
            helper: PathBuf::new(),
            name: "test".into(),
            curve: Curve::Ed25519,
            path: "m/0'".into(),
        };
        let mut response = json!({"name":"test", "curve":"ed25519", "path":"m/0'", "publicKey":"0xd75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"});
        let pair = parse_response(&selection, &key, response.to_string().as_bytes(), b"").unwrap();
        assert_eq!(pair.private.as_slice(), key);
        assert_eq!(pair.public, response["publicKey"].as_str().unwrap());
        assert!(
            parse_response(&selection, &key[..31], response.to_string().as_bytes(), b"").is_err()
        );
        response["curve"] = json!("secp256k1");
        assert!(parse_response(&selection, &key, response.to_string().as_bytes(), b"").is_err());
        response["curve"] = json!("ed25519");
        response["publicKey"] = json!(hex(&[0; 32]));
        assert!(parse_response(&selection, &key, response.to_string().as_bytes(), b"").is_err());
        assert_eq!(
            reported_error(b"untrusted exception and pretend secret"),
            "HELPER_FAILED"
        );
    }

    #[test]
    fn stalled_helper_and_inherited_pipes_do_not_outlive_timeout() {
        let root = folder();
        let helper = root.join("helper.sh");
        fs::write(&helper, "#!/bin/sh\n/bin/sleep 2\n").unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
        let selection = Selection {
            helper,
            name: "test".into(),
            curve: Curve::Ed25519,
            path: "m/0'".into(),
        };
        let start = Instant::now();
        assert!(matches!(
            read_key(&selection, Duration::from_millis(40)),
            Err("AUTHENTICATION_TIMEOUT")
        ));
        assert!(start.elapsed() < Duration::from_secs(1));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_unlock_is_sticky_in_the_process_cache() {
        let root = folder();
        let helper = root.join("helper.sh");
        fs::write(
            &helper,
            "#!/bin/sh\ncat >/dev/null\nprintf 'CANCELLED\\n' >&2\nexit 1\n",
        )
        .unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
        let selection = Selection {
            helper: helper.clone(),
            name: "test".into(),
            curve: Curve::Ed25519,
            path: "m/0'".into(),
        };
        assert!(matches!(unlock(selection.clone()), Err("CANCELLED")));
        fs::write(
            &helper,
            "#!/bin/sh\ncat >/dev/null\nprintf 'NOT_FOUND\\n' >&2\nexit 1\n",
        )
        .unwrap();
        assert!(matches!(unlock(selection), Err("CANCELLED")));
        fs::remove_dir_all(root).unwrap();
    }
}
