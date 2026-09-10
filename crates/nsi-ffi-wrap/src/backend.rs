//! Which ɴsɪ renderer a [`Context`](crate::Context) talks to.
//!
//! ɴsɪ is an interface, not a renderer, and more than one
//! implementation of it exists. This module is how a context says which
//! one it wants, at runtime, by name:
//!
//! ```no_run
//! # use nsi_ffi_wrap as nsi;
//! let context = nsi::Context::new(Some(&[
//!     nsi::string!("renderer", "moonray"),
//! ]));
//! ```
//!
//! # A name, or a library
//!
//! The name is matched against [`KNOWN`] first. Only when nothing
//! matches is it treated as a library to load, which is what lets an
//! implementation this crate has never heard of be used without a
//! release: `"mitsuba"` finds `libnsi_mitsuba.so` the day that library
//! exists.
//!
//! **A known name therefore cannot be redirected.** `"3delight"` means
//! the 3Delight this machine has, and a path that happens to end in
//! `3delight` will not shadow it. To point at a build of your own,
//! pass its path, or set that renderer's own prefix variable. This is
//! deliberate: a name meaning a predictable thing is worth more than
//! the convenience, and there are two escape hatches already.
//!
//! # Where the name comes from
//!
//! The `"renderer"` argument to
//! [`Context::new`](crate::Context::new), or `$NSI_RENDERER` when
//! there is none, or 3Delight when there is neither. The argument is
//! **consumed here and not forwarded**: it is a direction to this
//! crate about which library to open, and no renderer would know what
//! to do with it.
//!
//! An environment default is what lets an application be switched
//! between renderers without touching a call site, which is most of
//! the point of choosing at runtime at all.

use std::path::{Path, PathBuf};

/// The [`Context::new`](crate::Context::new) argument naming the
/// renderer.
pub const RENDERER: &str = "renderer";

/// The environment variable read when no `"renderer"` argument is
/// given.
pub const RENDERER_ENV: &str = "NSI_RENDERER";

/// What this crate knows about one ɴsɪ implementation.
///
/// Everything here is a fact about where a vendor installs, so a name
/// resolves on an ordinary machine with nothing set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backend {
    /// The canonical name, and what an error message prints.
    pub name: &'static str,
    /// Names that also reach it.
    pub aliases: &'static [&'static str],
    /// The library's file name on this platform.
    pub library: &'static str,
    /// An environment variable naming an install prefix, whose `lib`
    /// (`bin` on Windows) holds [`Backend::library`].
    pub prefix_var: &'static str,
    /// Where the vendor installs when nobody says otherwise. A prefix,
    /// not a file, so it is joined the same way as `prefix_var`.
    pub install_prefix: &'static str,
}

/// Every implementation this crate knows by name.
///
/// **3Delight is first and is the default**, which is what it was
/// before a renderer could be named at all.
///
/// The library names are not a pattern: 3Delight ships `lib3delight`
/// and the MoonRay backend ships `libnsi_moonray`, because the first
/// *is* a renderer and the second is an ɴsɪ front end onto one. That
/// is exactly why an unknown name tries both spellings.
pub const KNOWN: &[Backend] = &[
    Backend {
        name: "3delight",
        aliases: &["delight"],
        library: DELIGHT_LIBRARY,
        prefix_var: "DELIGHT",
        install_prefix: DELIGHT_PREFIX,
    },
    Backend {
        name: "moonray",
        // The crate is `nsi-moonray`; the renderer under it is
        // MoonRay. Both spellings reach the same library.
        aliases: &["nsi-moonray", "nsi_moonray"],
        library: MOONRAY_LIBRARY,
        // **Not `$MOONRAY_ROOT`**, which names DreamWorks' renderer.
        // This is the ɴsɪ front end onto it, and the two are installed
        // separately often enough that sharing one variable would make
        // "which MoonRay is this" unanswerable.
        prefix_var: "NSI_MOONRAY",
        install_prefix: MOONRAY_PREFIX,
    },
];

#[cfg(target_os = "linux")]
const DELIGHT_LIBRARY: &str = "lib3delight.so";
#[cfg(target_os = "macos")]
const DELIGHT_LIBRARY: &str = "lib3delight.dylib";
#[cfg(target_os = "windows")]
const DELIGHT_LIBRARY: &str = "3Delight.dll";

#[cfg(target_os = "linux")]
const DELIGHT_PREFIX: &str = "/usr/local/3delight";
#[cfg(target_os = "macos")]
const DELIGHT_PREFIX: &str = "/Applications/3Delight";
#[cfg(target_os = "windows")]
const DELIGHT_PREFIX: &str = "C:/%ProgramFiles%/3Delight";

#[cfg(target_os = "linux")]
const MOONRAY_LIBRARY: &str = "libnsi_moonray.so";
#[cfg(target_os = "macos")]
const MOONRAY_LIBRARY: &str = "libnsi_moonray.dylib";
#[cfg(target_os = "windows")]
const MOONRAY_LIBRARY: &str = "nsi_moonray.dll";

#[cfg(target_os = "linux")]
const MOONRAY_PREFIX: &str = "/usr/local/nsi-moonray";
#[cfg(target_os = "macos")]
const MOONRAY_PREFIX: &str = "/Applications/nsi-moonray";
#[cfg(target_os = "windows")]
const MOONRAY_PREFIX: &str = "C:/%ProgramFiles%/nsi-moonray";

/// Where a library lives under an install prefix.
///
/// Unix puts a shared library in `lib`; Windows puts a DLL next to the
/// executables that load it.
#[cfg(not(target_os = "windows"))]
const LIBRARY_DIRECTORY: &str = "lib";
#[cfg(target_os = "windows")]
const LIBRARY_DIRECTORY: &str = "bin";

/// The backend a name refers to, if this crate knows one.
///
/// Case-insensitive, because the name usually arrives from a
/// configuration file a person typed.
pub fn lookup(name: &str) -> Option<&'static Backend> {
    let wanted = name.trim().to_ascii_lowercase();
    KNOWN.iter().find(|backend| {
        backend.name == wanted
            || backend.aliases.iter().any(|alias| *alias == wanted)
    })
}

/// Every canonical name, for an error message that has to list them.
pub fn known_names() -> impl Iterator<Item = &'static str> {
    KNOWN.iter().map(|backend| backend.name)
}

impl Backend {
    /// Where to look for this backend's library, best answer first.
    ///
    /// **What was said explicitly wins.** The prefix variable is first
    /// so that a machine with both a system install and a
    /// `$DELIGHT` pointing elsewhere uses the one it was pointed at.
    /// The reverse order renders, with the wrong renderer, and says
    /// nothing -- which is a worse afternoon than not rendering.
    ///
    /// Then a bundle, found relative to the running executable, so a
    /// relocatable copy works with no environment at all. Then the
    /// vendor's install prefix. Then the bare file name, which is the
    /// system loader's own search and covers a distribution package.
    pub fn candidates(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();

        if let Some(prefix) = std::env::var_os(self.prefix_var) {
            // **Both, because people point these two ways.** The
            // vendor convention is a prefix whose `lib` holds the
            // library, and that is tried first. But a variable set to
            // the directory the library is actually in is the other
            // thing everyone does -- a Cargo `target/debug`, say --
            // and failing on it would be a bad afternoon for no
            // reason.
            let prefix = Path::new(&prefix);
            paths.push(prefix.join(LIBRARY_DIRECTORY).join(self.library));
            paths.push(prefix.join(self.library));
        }

        paths.extend(beside_executable(self.library));

        paths.push(
            Path::new(self.install_prefix)
                .join(LIBRARY_DIRECTORY)
                .join(self.library),
        );
        paths.push(PathBuf::from(self.library));

        paths
    }
}

/// Where to look for a name this crate does not know, best answer
/// first.
///
/// Verbatim first, so an absolute path and a full soname both work.
/// Then the two spellings a backend library actually uses. `libnsi_`
/// comes first because a name that is not known here is far more
/// likely to be a *new* ɴsɪ front end than a renderer that predates
/// this table.
pub fn library_candidates(name: &str) -> Vec<PathBuf> {
    let name = name.trim();
    let mut paths = vec![PathBuf::from(name)];

    // A path is already the answer; decorating it would only produce
    // nonsense like `libnsi_/opt/foo/libbar.so`.
    if name.contains(['/', '\\']) {
        return paths;
    }

    for stem in [format!("nsi_{name}"), name.to_string()] {
        paths.push(PathBuf::from(library_file_name(&stem)));
        paths.extend(beside_executable(&library_file_name(&stem)));
    }

    paths
}

/// One library file name, spelled the way this platform spells it.
#[cfg(target_os = "linux")]
fn library_file_name(stem: &str) -> String {
    format!("lib{stem}.so")
}
#[cfg(target_os = "macos")]
fn library_file_name(stem: &str) -> String {
    format!("lib{stem}.dylib")
}
#[cfg(target_os = "windows")]
fn library_file_name(stem: &str) -> String {
    format!("{stem}.dll")
}

/// A bundle's library directory, relative to the running executable.
///
/// A relocatable install puts the binary in `bin` and the libraries in
/// `lib` beside it, so `../lib` from the executable is the bundle's
/// own copy. Looked at before the system locations, so unpacking a
/// bundle somewhere and running it does not quietly pick up an older
/// system-wide library instead.
fn beside_executable(library: &str) -> Vec<PathBuf> {
    let Ok(executable) = std::env::current_exe() else {
        return Vec::new();
    };
    let Some(directory) = executable.parent() else {
        return Vec::new();
    };

    let mut paths = Vec::new();
    if let Some(prefix) = directory.parent() {
        paths.push(prefix.join(LIBRARY_DIRECTORY).join(library));
    }
    paths.push(directory.join(library));
    paths
}

/// Where to look, for any name at all.
///
/// The one place the "known first, then a library" rule is written
/// down, so the loader and the error message cannot disagree about it.
pub fn candidates(name: &str) -> Vec<PathBuf> {
    match lookup(name) {
        Some(backend) => backend.candidates(),
        None => library_candidates(name),
    }
}

/// The renderer this process should use when nothing asked for one.
///
/// `$NSI_RENDERER`, or 3Delight. Read on each call rather than cached,
/// so a test that sets it is not at the mercy of what ran before it.
pub fn from_environment() -> Option<String> {
    std::env::var(RENDERER_ENV)
        .ok()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The names in the table resolve, including their aliases and
    /// whatever case someone typed.
    #[test]
    fn a_known_name_is_found() {
        assert_eq!(lookup("3delight").map(|b| b.name), Some("3delight"));
        assert_eq!(lookup("delight").map(|b| b.name), Some("3delight"));
        assert_eq!(lookup("MoonRay").map(|b| b.name), Some("moonray"));
        assert_eq!(lookup("  moonray  ").map(|b| b.name), Some("moonray"));
        assert_eq!(lookup("nsi-moonray").map(|b| b.name), Some("moonray"));
    }

    /// An unknown name is a library, which is what keeps the set open.
    #[test]
    fn an_unknown_name_is_not_found() {
        assert!(lookup("mitsuba").is_none());
        assert!(lookup("").is_none());
    }

    /// **Both spellings**, because the two backends this crate ships
    /// with do not agree on one: `lib3delight` against
    /// `libnsi_moonray`.
    #[test]
    fn an_unknown_name_tries_both_library_spellings() {
        let paths = library_candidates("mitsuba");
        let names: Vec<String> = paths
            .iter()
            .filter_map(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect();

        assert!(names.iter().any(|name| name == "mitsuba"), "{names:?}");
        assert!(
            names.iter().any(|name| name.contains("nsi_mitsuba")),
            "{names:?}"
        );
        assert!(
            names
                .iter()
                .any(|name| name.contains("mitsuba") && !name.contains("nsi_")
                    && name != "mitsuba"),
            "{names:?}"
        );
    }

    /// A path is taken as one and never decorated: `libnsi_/opt/x.so`
    /// is not a file anyone meant.
    #[test]
    fn a_path_is_left_alone() {
        let paths = library_candidates("/opt/renderer/libfoo.so");
        assert_eq!(paths, vec![PathBuf::from("/opt/renderer/libfoo.so")]);
    }

    /// A known name resolves through its own table rather than as a
    /// library, which is the rule the whole module turns on.
    #[test]
    fn a_known_name_does_not_go_through_the_library_search() {
        let known = candidates("3delight");
        assert!(
            known.iter().any(|path| path.ends_with(DELIGHT_LIBRARY)),
            "{known:?}"
        );

        // And an unknown one does.
        let unknown = candidates("nothing-by-this-name");
        assert_eq!(unknown[0], PathBuf::from("nothing-by-this-name"));
    }

    /// A prefix variable is honoured whether it names an install
    /// prefix or the directory the library is in.
    #[test]
    fn a_prefix_variable_is_read_both_ways() {
        // SAFETY: single-threaded test; restored before returning.
        let previous = std::env::var_os("NSI_MOONRAY");
        unsafe { std::env::set_var("NSI_MOONRAY", "/tmp/checkout") };

        let paths = lookup("moonray").expect("moonray is known").candidates();

        match previous {
            Some(value) => unsafe {
                std::env::set_var("NSI_MOONRAY", value)
            },
            None => unsafe { std::env::remove_var("NSI_MOONRAY") },
        }

        assert!(
            paths.contains(&PathBuf::from("/tmp/checkout/lib").join(
                MOONRAY_LIBRARY
            )),
            "the install-prefix shape: {paths:?}"
        );
        assert!(
            paths.contains(&PathBuf::from("/tmp/checkout").join(
                MOONRAY_LIBRARY
            )),
            "the library-directory shape: {paths:?}"
        );
    }

    /// What was said explicitly is looked at before what is installed.
    #[test]
    fn the_prefix_variable_wins() {
        // SAFETY: single-threaded test, and the variable is read back
        // immediately. Restored before returning.
        let previous = std::env::var_os("DELIGHT");
        unsafe { std::env::set_var("DELIGHT", "/tmp/a-3delight-of-my-own") };

        let paths = lookup("3delight").expect("3delight is known").candidates();

        match previous {
            Some(value) => unsafe { std::env::set_var("DELIGHT", value) },
            None => unsafe { std::env::remove_var("DELIGHT") },
        }

        assert!(
            paths[0].starts_with("/tmp/a-3delight-of-my-own"),
            "the prefix variable should be looked at first: {paths:?}"
        );
    }
}
