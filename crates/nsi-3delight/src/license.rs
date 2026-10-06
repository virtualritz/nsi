//! The 3Delight license server.
//!
//! 3Delight's **free** license is one seat per machine, held in process:
//! a second 3Delight process on the same host is refused with `Cannot
//! run more than one 3Delight free license at once`. A machine with a
//! **commercial** license runs `licserver`, a daemon that hands out
//! seats; with it up, several renderers coexist.
//!
//! This module answers the question an embedding application asks before
//! its first render -- [`license_server_request`] -- and can
//! [`start_license_server`].
//!
//! Nothing here starts or stops a server on its own. The caller decides,
//! because a server is machine-wide and outlives any one application.

use std::{
    env, fs, io,
    path::PathBuf,
    process::{Command, Stdio},
};

/// Where 3Delight is installed, from the `DELIGHT` environment variable.
///
/// `None` when `DELIGHT` is unset, which is how a machine without
/// 3Delight reports itself.
#[must_use]
pub fn delight_root() -> Option<PathBuf> {
    env::var_os("DELIGHT").map(PathBuf::from)
}

/// A license configured for this machine.
///
/// Either a file 3Delight would read, or a license server it would
/// contact. The distinction matters to [`license_server_request`]: a
/// file is served by the local `licserver`, a server is not.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LicenseSource {
    /// A license file on this machine.
    File(PathBuf),
    /// A license server, as `port@host` or a bare host name.
    Server(String),
}

/// The license 3Delight would use, if one is configured.
///
/// In order:
///
/// 1. `RLM_LICENSE`, which the embedded RLM client reads. Its entries
///    are `:`- or `;`-separated; an entry containing `@` is a server,
///    otherwise it is a file path. A server anywhere in the list wins
///    over a file, because a configured server is the whole setup.
/// 2. `$DELIGHT/licenses/3delight_license.dat`, the installed license.
/// 3. `$HOME/.config/3delight/license.dat`, the per-user file.
/// 4. The `license.server` key in `$DELIGHT/3delight.config`.
///
/// A machine with none of these is on the free tier, which needs no
/// server.
#[must_use]
pub fn license_source() -> Option<LicenseSource> {
    configured_license()
        .or_else(installed_license)
        .or_else(per_user_license)
        .or_else(configured_server)
}

/// The license file 3Delight would read, if [`license_source`] named a
/// file.
///
/// `None` when no license is configured, or when the configured source
/// is a server rather than a file.
#[must_use]
pub fn license_file() -> Option<PathBuf> {
    match license_source() {
        Some(LicenseSource::File(path)) => Some(path),
        Some(LicenseSource::Server(_)) | None => None,
    }
}

/// Whether `licutils serverstatus` reaches a license server.
///
/// The only signal is the tool's own output: with no server it writes
/// `cannot connect to server '...'` to stderr and still exits `0`. A
/// missing `DELIGHT` or `licutils` is not running.
#[must_use]
pub fn is_license_server_running() -> bool {
    let Some(root) = delight_root() else {
        return false;
    };
    let Ok(output) = Command::new(root.join("bin").join("licutils"))
        .arg("serverstatus")
        .output()
    else {
        return false;
    };
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    !text.contains("cannot connect")
}

/// Starts the license server as a daemon.
///
/// Runs `$DELIGHT/bin/licserver -d` with no inherited stdio.
/// `licserver` forks itself, so the call returns once the process is
/// spawned and the server keeps running after the caller exits.
pub fn start_license_server() -> io::Result<()> {
    let root = delight_root().ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "DELIGHT is not set")
    })?;
    let server = root.join("bin").join("licserver");
    if !server.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no license server at {}", server.display()),
        ));
    }
    Command::new(server)
        .arg("-d")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

/// A license server that is installed and licensed but not running.
///
/// The answer [`license_server_request`] returns, and what the caller
/// needs to act on it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LicenseServerRequest {
    /// The `DELIGHT` root holding the server binary.
    pub delight_root: PathBuf,
    /// The license file the server would serve.
    pub license_file: PathBuf,
}

/// Whether to offer to start the license server.
///
/// `Some` when 3Delight is installed, a local license file is
/// configured and `licutils serverstatus` does not answer; `None`
/// otherwise. The free tier has no license, so it is never offered; a
/// configured server is not the local server's business, so it is not
/// offered either.
#[must_use]
pub fn license_server_request() -> Option<LicenseServerRequest> {
    license_server_request_for(
        delight_root(),
        license_file(),
        is_license_server_running(),
    )
}

/// The whole policy, as a pure function of the environment's answers.
fn license_server_request_for(
    delight_root: Option<PathBuf>,
    license_file: Option<PathBuf>,
    server_is_running: bool,
) -> Option<LicenseServerRequest> {
    if server_is_running {
        None
    } else {
        Some(LicenseServerRequest {
            delight_root: delight_root?,
            license_file: license_file?,
        })
    }
}

/// The `RLM_LICENSE` list, classified. A server entry wins over a file.
fn configured_license() -> Option<LicenseSource> {
    license_source_from_rlm(&env::var("RLM_LICENSE").ok()?)
}

/// [`configured_license`] without the environment, so it is testable.
fn license_source_from_rlm(value: &str) -> Option<LicenseSource> {
    let entries: Vec<&str> = value
        .split([':', ';'])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .collect();
    entries
        .iter()
        .find(|entry| entry.contains('@'))
        .map(|entry| LicenseSource::Server((*entry).to_string()))
        .or_else(|| {
            entries
                .iter()
                .map(|entry| PathBuf::from(*entry))
                .find(|path| path.is_file())
                .map(LicenseSource::File)
        })
}

/// `$DELIGHT/licenses/3delight_license.dat`, if present.
fn installed_license() -> Option<LicenseSource> {
    let path = delight_root()?
        .join("licenses")
        .join("3delight_license.dat");
    path.is_file().then_some(LicenseSource::File(path))
}

/// `$HOME/.config/3delight/license.dat`, if present.
fn per_user_license() -> Option<LicenseSource> {
    let path = PathBuf::from(env::var_os("HOME")?)
        .join(".config")
        .join("3delight")
        .join("license.dat");
    path.is_file().then_some(LicenseSource::File(path))
}

/// The `license.server` key of `$DELIGHT/3delight.config`, if set.
fn configured_server() -> Option<LicenseSource> {
    let text =
        fs::read_to_string(delight_root()?.join("3delight.config")).ok()?;
    text.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .find_map(|line| {
            let (key, value) = line.split_once(|c: char| c.is_whitespace())?;
            (key == "license.server")
                .then(|| LicenseSource::Server(value.trim().to_string()))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(text: &str) -> PathBuf {
        PathBuf::from(text)
    }

    #[test]
    fn a_port_at_host_rlm_entry_is_a_server() {
        assert_eq!(
            license_source_from_rlm("5053@licsrv"),
            Some(LicenseSource::Server("5053@licsrv".to_string()))
        );
    }

    #[test]
    fn an_rlm_list_prefers_a_server_over_a_file() {
        assert_eq!(
            license_source_from_rlm("Cargo.toml:5053@licsrv"),
            Some(LicenseSource::Server("5053@licsrv".to_string()))
        );
    }

    #[test]
    fn an_existing_rlm_file_entry_is_a_file() {
        assert_eq!(
            license_source_from_rlm("Cargo.toml"),
            Some(LicenseSource::File(path("Cargo.toml")))
        );
    }

    #[test]
    fn an_rlm_entry_that_is_neither_is_none() {
        assert_eq!(license_source_from_rlm("does-not-exist.lic"), None);
        assert_eq!(license_source_from_rlm(""), None);
    }

    #[test]
    fn no_root_no_request() {
        assert!(
            license_server_request_for(
                None,
                Some(path("/x/license.dat")),
                false
            )
            .is_none()
        );
    }

    #[test]
    fn no_license_no_request() {
        assert!(
            license_server_request_for(Some(path("/delight")), None, false)
                .is_none()
        );
    }

    #[test]
    fn running_server_no_request() {
        assert!(
            license_server_request_for(
                Some(path("/delight")),
                Some(path("/x/license.dat")),
                true
            )
            .is_none()
        );
    }

    #[test]
    fn installed_licensed_and_down_requests() {
        let request = license_server_request_for(
            Some(path("/delight")),
            Some(path("/x/license.dat")),
            false,
        )
        .expect("all three inputs are present");
        assert_eq!(request.delight_root, path("/delight"));
        assert_eq!(request.license_file, path("/x/license.dat"));
    }

    #[test]
    fn the_environment_probes_do_not_panic() {
        let _ = delight_root();
        let _ = license_source();
        let _ = license_file();
        let _ = is_license_server_running();
        let _ = license_server_request();
    }
}
