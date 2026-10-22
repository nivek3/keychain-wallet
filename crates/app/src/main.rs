mod catalog;
mod macos;
mod model;
mod theme;
mod ui;

use model::{Curve, Failure, KeyRequest, Result};
use std::{
    io::{Read, Write},
    os::fd::FromRawFd,
};

fn emit(value: &serde_json::Value) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, value).map_err(|_| Failure::Transport)?;
    stdout.write_all(b"\n").map_err(|_| Failure::Transport)
}
fn pipe_descriptor(fd: i32) -> bool {
    unsafe {
        let mut stat = std::mem::zeroed::<libc::stat>();
        if libc::fstat(fd, &mut stat) != 0 {
            return false;
        }
        matches!(stat.st_mode & libc::S_IFMT, libc::S_IFIFO | libc::S_IFSOCK)
    }
}
fn read_request(input: &mut impl Read) -> Result<KeyRequest> {
    let mut data = Vec::new();
    input
        .take(2049)
        .read_to_end(&mut data)
        .map_err(|_| Failure::Transport)?;
    if data.len() > 2048 {
        return Err(Failure::Input);
    }
    let request: KeyRequest = serde_json::from_slice(&data).map_err(|_| Failure::Input)?;
    request.validate()?;
    Ok(request)
}
fn main_impl() -> Result<()> {
    unsafe {
        let limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if libc::setrlimit(libc::RLIMIT_CORE, &limit) != 0 {
            return Err(Failure::Configuration);
        }
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let vault = macos::Vault::new()?;
    if args == ["check-signing"] {
        return emit(
            &serde_json::json!({"status":"SIGNING_CONFIGURATION_OK","accessGroup":vault.group}),
        );
    }
    if args.is_empty() {
        return ui::run(vault, None, None);
    }
    if args == ["--bridge"] {
        if !pipe_descriptor(0) || !pipe_descriptor(3) {
            return Err(Failure::Transport);
        }
        let request = read_request(&mut std::io::stdin().lock())?;
        macos::initialize_accessory()?;
        let context = macos::AuthContext::new(
            &format!(
                "Wallet: {}\nCurve: {}\nPath: {}",
                request.name,
                request.curve.name(),
                request.path
            ),
            false,
        );
        let wallet = vault.read(&request.name, &context)?;
        let pair = model::derive(&wallet, &request, None)?;
        // The private key never goes to stdout, stderr, argv, or an environment variable.
        let mut secret_pipe = unsafe { std::fs::File::from_raw_fd(3) };
        secret_pipe
            .write_all(&*pair.key)
            .map_err(|_| Failure::Transport)?;
        return emit(
            &serde_json::json!({"name":request.name,"path":request.path,"curve":request.curve.name(),"publicKey":pair.public_key}),
        );
    }
    if args == ["list"] {
        macos::initialize_accessory()?;
        for entry in catalog::Catalog::for_group(&vault.group)?.load()? {
            emit(&serde_json::json!({"name":entry.name}))?;
        }
        return Ok(());
    }
    if args.len() == 2 && args[0] == "delete" {
        model::validate_name(&args[1])?;
        return ui::run(vault, None, Some(args[1].clone()));
    }
    if args.len() != 4 || !matches!(args[0].as_str(), "import" | "address") {
        return Err(Failure::Input);
    }
    let request = KeyRequest {
        name: args[1].clone(),
        path: args[2].clone(),
        curve: Curve::Secp256k1,
    };
    request.validate_identifier(&args[3])?;
    if args[0] == "import" {
        return ui::run(vault, Some((request, args[3].clone())), None);
    }
    macos::initialize_accessory()?;
    let context = macos::AuthContext::new(
        &format!(
            "Wallet: {}\nPath: {}\nExpected address: {}",
            request.name, request.path, args[3]
        ),
        false,
    );
    let wallet = vault.read(&request.name, &context)?;
    let pair = model::derive(&wallet, &request, Some(&args[3]))?;
    emit(&serde_json::json!({"name":request.name,"path":request.path,"address":pair.identifier}))
}
fn main() {
    // Suppress panic payloads and backtraces, which may contain secret-bearing inputs.
    std::panic::set_hook(Box::new(|_| eprintln!("INVALID_INPUT")));
    let outcome = objc2::rc::autoreleasepool(|_| main_impl());
    if let Err(error) = outcome {
        eprintln!("{}", error.code());
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bridge_request_is_bounded_and_does_not_accept_secret_arguments() {
        let request = br#"{"name":"public-test","curve":"ed25519","path":"m/0'"}"#;
        assert!(read_request(&mut request.as_slice()).is_ok());
        assert!(read_request(&mut vec![b' '; 2049].as_slice()).is_err());
        assert!(read_request(&mut br#"{"name":"public-test","path":"m/0'"}"#.as_slice()).is_err());
        assert!(read_request(
            &mut br#"{"name":"public-test","curve":"ed25519","path":"m/0'/1"}"#.as_slice()
        )
        .is_err());
    }
    #[test]
    fn secret_transport_rejects_regular_files() {
        let exe = std::env::current_exe().unwrap();
        let file = std::fs::File::open(exe).unwrap();
        use std::os::fd::AsRawFd;
        assert!(!pipe_descriptor(file.as_raw_fd()));
        let (a, _b) = std::os::unix::net::UnixStream::pair().unwrap();
        assert!(pipe_descriptor(a.as_raw_fd()));
    }
}
