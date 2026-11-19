//! Cross-platform Chromium Embedded Framework (CEF) engine SDK & downloader.
//!
//! Provides [`EngineSdk`] so client applications can check whether the CEF
//! engine is installed locally, download it on demand (for the current host or
//! any target platform), and resolve runtime paths without requiring the user
//! to pre-install CEF before launching the app.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::{Error, Result};

/// The `cef` crate version string pinned by this workspace.
pub const CEF_PACKAGE_VERSION: &str = "156.6.0+156.0.30";

/// Supported desktop targets for the CEF engine distribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineTarget {
    /// `x86_64-unknown-linux-gnu`
    LinuxX86_64,
    /// `aarch64-unknown-linux-gnu`
    LinuxAarch64,
    /// `x86_64-pc-windows-msvc`
    WindowsX86_64,
    /// `aarch64-pc-windows-msvc`
    WindowsAarch64,
    /// `aarch64-apple-darwin`
    MacosAarch64,
    /// `x86_64-apple-darwin`
    MacosX86_64,
}

impl EngineTarget {
    /// All supported desktop targets across Linux, Windows, and macOS.
    pub const ALL: [Self; 6] = [
        Self::LinuxX86_64,
        Self::LinuxAarch64,
        Self::WindowsX86_64,
        Self::WindowsAarch64,
        Self::MacosAarch64,
        Self::MacosX86_64,
    ];

    /// Returns the target matching the current host OS and architecture, if supported.
    #[must_use]
    pub const fn current() -> Option<Self> {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            Some(Self::LinuxX86_64)
        }
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        {
            Some(Self::LinuxAarch64)
        }
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        {
            Some(Self::WindowsX86_64)
        }
        #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
        {
            Some(Self::WindowsAarch64)
        }
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            Some(Self::MacosAarch64)
        }
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        {
            Some(Self::MacosX86_64)
        }
        #[cfg(not(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(
                target_os = "windows",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            all(
                target_os = "macos",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
        )))]
        {
            None
        }
    }

    /// Rust target triple used by `download-cef`.
    #[must_use]
    pub const fn triple(self) -> &'static str {
        match self {
            Self::LinuxX86_64 => "x86_64-unknown-linux-gnu",
            Self::LinuxAarch64 => "aarch64-unknown-linux-gnu",
            Self::WindowsX86_64 => "x86_64-pc-windows-msvc",
            Self::WindowsAarch64 => "aarch64-pc-windows-msvc",
            Self::MacosAarch64 => "aarch64-apple-darwin",
            Self::MacosX86_64 => "x86_64-apple-darwin",
        }
    }

    /// Extracted directory name produced by `download-cef` (`cef_<os>_<arch>`).
    #[must_use]
    pub const fn dir_name(self) -> &'static str {
        match self {
            Self::LinuxX86_64 => "cef_linux_x86_64",
            Self::LinuxAarch64 => "cef_linux_aarch64",
            Self::WindowsX86_64 => "cef_windows_x86_64",
            Self::WindowsAarch64 => "cef_windows_aarch64",
            Self::MacosAarch64 => "cef_macos_aarch64",
            Self::MacosX86_64 => "cef_macos_x86_64",
        }
    }

    /// Short OS family identifier (`"linux"`, `"windows"`, `"macos"`).
    #[must_use]
    pub const fn os(self) -> &'static str {
        match self {
            Self::LinuxX86_64 | Self::LinuxAarch64 => "linux",
            Self::WindowsX86_64 | Self::WindowsAarch64 => "windows",
            Self::MacosAarch64 | Self::MacosX86_64 => "macos",
        }
    }

    /// Parses a target specification string into one or more [`EngineTarget`]s.
    pub fn parse_targets(spec: &str) -> Result<Vec<Self>> {
        let mut out = Vec::new();
        for raw in spec.split(',') {
            let token = raw.trim().to_ascii_lowercase();
            if token.is_empty() {
                continue;
            }
            let slice: &[Self] = match token.as_str() {
                "all" => &Self::ALL,
                "host" | "current" => {
                    let current = Self::current().ok_or(Error::Unsupported(
                        "current host is not a supported CEF target",
                    ))?;
                    if !out.contains(&current) {
                        out.push(current);
                    }
                    continue;
                }
                "linux" => &[Self::LinuxX86_64, Self::LinuxAarch64],
                "windows" | "win" => &[Self::WindowsX86_64, Self::WindowsAarch64],
                "macos" | "mac" | "darwin" => &[Self::MacosAarch64, Self::MacosX86_64],
                "linux-x64" | "linux-x86_64" | "x86_64-unknown-linux-gnu" => &[Self::LinuxX86_64],
                "linux-arm64" | "linux-aarch64" | "aarch64-unknown-linux-gnu" => {
                    &[Self::LinuxAarch64]
                }
                "windows-x64" | "windows-x86_64" | "win-x64" | "x86_64-pc-windows-msvc" => {
                    &[Self::WindowsX86_64]
                }
                "windows-arm64" | "windows-aarch64" | "win-arm64" | "aarch64-pc-windows-msvc" => {
                    &[Self::WindowsAarch64]
                }
                "macos-arm64" | "macos-aarch64" | "mac-arm64" | "aarch64-apple-darwin" => {
                    &[Self::MacosAarch64]
                }
                "macos-x64" | "macos-x86_64" | "mac-x64" | "x86_64-apple-darwin" => {
                    &[Self::MacosX86_64]
                }
                other => {
                    return Err(Error::Backend(format!(
                        "unknown CEF engine target '{other}' (expected all, host, linux, windows, macos, or a target triple)"
                    )));
                }
            };
            for &target in slice {
                if !out.contains(&target) {
                    out.push(target);
                }
            }
        }

        if out.is_empty() {
            return Err(Error::Backend("no CEF engine target specified".to_string()));
        }

        Ok(out)
    }
}

impl std::fmt::Display for EngineTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.triple())
    }
}

/// Checks whether `dir` contains a usable CEF engine runtime for its platform.
#[must_use]
pub fn is_engine_dir_valid(dir: &Path) -> bool {
    if dir.join("Chromium Embedded Framework.framework").is_dir() {
        return true;
    }
    dir.join("icudtl.dat").is_file()
        && dir.join("resources.pak").is_file()
        && dir.join("locales").is_dir()
}

/// Client SDK for managing on-demand CEF engine installations across platforms.
///
/// Allows a desktop client to start immediately without a bundled CEF engine,
/// check whether the engine is available locally, and download/activate it on
/// demand from the UI or CLI.
#[derive(Debug, Clone)]
pub struct EngineSdk {
    root_dir: PathBuf,
}

impl EngineSdk {
    /// Creates an [`EngineSdk`] rooted at `root_dir` (e.g. `~/.unotrop/engines`).
    #[must_use]
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        Self {
            root_dir: root_dir.into(),
        }
    }

    /// Returns the default user-level engine storage directory (`~/.unotrop/engines`).
    #[must_use]
    pub fn default_dir() -> PathBuf {
        if let Some(home) = std::env::var_os("UNOTROP_HOME").filter(|p| !p.is_empty()) {
            return PathBuf::from(home).join("engines");
        }
        if let Some(home) = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .filter(|p| !p.is_empty())
        {
            return PathBuf::from(home).join(".unotrop").join("engines");
        }
        std::env::temp_dir().join("gpui-cef-engines")
    }

    /// Root directory where downloaded `cef_<os>_<arch>` folders are stored.
    #[must_use]
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    /// Expected installation directory for a specific `target`.
    #[must_use]
    pub fn target_dir(&self, target: EngineTarget) -> PathBuf {
        self.root_dir.join(target.dir_name())
    }

    /// Returns `true` if `target` is already downloaded and valid in `self.root_dir`.
    #[must_use]
    pub fn is_installed(&self, target: EngineTarget) -> bool {
        is_engine_dir_valid(&self.target_dir(target))
    }

    /// Resolves the active CEF engine directory for the current host machine,
    /// checking in order:
    /// 1. `self.root_dir/<host_target>` (downloaded via SDK)
    /// 2. `CEF_PATH` environment variable
    /// 3. Directory next to the current executable (when running from a bundle)
    #[must_use]
    pub fn resolve_host_engine_dir(&self) -> Option<PathBuf> {
        if let Some(target) = EngineTarget::current() {
            let sdk_dir = self.target_dir(target);
            if is_engine_dir_valid(&sdk_dir) {
                return Some(sdk_dir);
            }
        }
        if let Some(env_dir) = std::env::var_os("CEF_PATH")
            .map(PathBuf::from)
            .filter(|dir| is_engine_dir_valid(dir))
        {
            return Some(env_dir);
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                if is_engine_dir_valid(exe_dir) {
                    return Some(exe_dir.to_path_buf());
                }
                #[cfg(target_os = "macos")]
                if let Some(contents_dir) = exe_dir.parent() {
                    let frameworks = contents_dir.join("Frameworks");
                    if is_engine_dir_valid(&frameworks) {
                        return Some(frameworks);
                    }
                }
            }
        }
        None
    }

    /// Returns `true` if a usable CEF engine is available for the current host
    /// (either via this SDK, `CEF_PATH`, or bundled next to the executable).
    #[must_use]
    pub fn is_host_available(&self) -> bool {
        self.resolve_host_engine_dir().is_some()
    }

    /// Downloads and extracts the CEF engine for `target` into `self.root_dir`.
    pub fn install(&self, target: EngineTarget, show_progress: bool) -> Result<PathBuf> {
        download_engine(target, &self.root_dir, show_progress)
    }

    /// Downloads and extracts the CEF engine for the current host OS and architecture.
    pub fn install_host(&self, show_progress: bool) -> Result<PathBuf> {
        let target = EngineTarget::current().ok_or(Error::Unsupported(
            "current host is not a supported CEF target",
        ))?;
        download_engine(target, &self.root_dir, show_progress)
    }

    /// Downloads and extracts the CEF engine for multiple targets, fetching the index once.
    pub fn install_many(
        &self,
        targets: &[EngineTarget],
        show_progress: bool,
    ) -> Result<Vec<(EngineTarget, PathBuf)>> {
        download_engines(targets, &self.root_dir, show_progress)
    }

    /// Downloads and extracts the CEF engine for all 6 desktop targets.
    pub fn install_all(&self, show_progress: bool) -> Result<Vec<(EngineTarget, PathBuf)>> {
        download_all_engines(&self.root_dir, show_progress)
    }

    /// Removes the downloaded engine for `target` from `self.root_dir`, if present.
    pub fn uninstall(&self, target: EngineTarget) -> Result<()> {
        let dir = self.target_dir(target);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|err| {
                Error::Backend(format!("failed to remove {}: {err}", dir.display()))
            })?;
        }
        Ok(())
    }
}

impl Default for EngineSdk {
    fn default() -> Self {
        Self::new(Self::default_dir())
    }
}

/// Downloads and extracts the CEF engine for a single [`EngineTarget`] into `output_dir`.
///
/// If a matching verified `archive.json` already exists in `<output_dir>/<target.dir_name()>`,
/// the existing extraction is reused without re-downloading.
pub fn download_engine(
    target: EngineTarget,
    output_dir: &Path,
    show_progress: bool,
) -> Result<PathBuf> {
    let cef_version = download_cef::default_version(CEF_PACKAGE_VERSION);
    let expected_dir = output_dir.join(target.dir_name());

    if expected_dir.exists()
        && is_engine_dir_valid(&expected_dir)
        && download_cef::check_archive_json(&cef_version, &expected_dir.to_string_lossy()).is_ok()
    {
        return Ok(expected_dir);
    }

    let index = download_cef::CefIndex::download()
        .map_err(|err| Error::Backend(format!("failed to download CEF index: {err}")))?;
    download_engine_with_index(&index, &cef_version, target, output_dir, show_progress)
}

/// Downloads and extracts the CEF engine for every target in `targets` into `output_dir`,
/// fetching the remote CEF release index at most once.
pub fn download_engines(
    targets: &[EngineTarget],
    output_dir: &Path,
    show_progress: bool,
) -> Result<Vec<(EngineTarget, PathBuf)>> {
    std::fs::create_dir_all(output_dir).map_err(|err| {
        Error::Backend(format!("failed to create {}: {err}", output_dir.display()))
    })?;

    let cef_version = download_cef::default_version(CEF_PACKAGE_VERSION);
    let mut cached_index: Option<download_cef::CefIndex> = None;
    let mut results = Vec::with_capacity(targets.len());

    for &target in targets {
        let expected_dir = output_dir.join(target.dir_name());
        if expected_dir.exists()
            && is_engine_dir_valid(&expected_dir)
            && download_cef::check_archive_json(&cef_version, &expected_dir.to_string_lossy())
                .is_ok()
        {
            results.push((target, expected_dir));
            continue;
        }

        let index = match &cached_index {
            Some(index) => index,
            None => {
                let fetched = download_cef::CefIndex::download().map_err(|err| {
                    Error::Backend(format!("failed to download CEF index: {err}"))
                })?;
                cached_index.insert(fetched)
            }
        };

        let extracted =
            download_engine_with_index(index, &cef_version, target, output_dir, show_progress)?;
        results.push((target, extracted));
    }

    Ok(results)
}

/// Downloads and extracts the CEF engine for all supported desktop platforms
/// (Linux x86_64/arm64, Windows x86_64/arm64, macOS arm64/x86_64).
pub fn download_all_engines(
    output_dir: &Path,
    show_progress: bool,
) -> Result<Vec<(EngineTarget, PathBuf)>> {
    download_engines(&EngineTarget::ALL, output_dir, show_progress)
}

fn download_engine_with_index(
    index: &download_cef::CefIndex,
    cef_version: &str,
    target: EngineTarget,
    output_dir: &Path,
    show_progress: bool,
) -> Result<PathBuf> {
    std::fs::create_dir_all(output_dir).map_err(|err| {
        Error::Backend(format!("failed to create {}: {err}", output_dir.display()))
    })?;

    let download_url = download_cef::default_download_url();
    let platform = index
        .platform(target.triple())
        .map_err(|err| Error::Backend(format!("CEF platform lookup failed for {target}: {err}")))?;
    let version = platform.version(cef_version).map_err(|err| {
        Error::Backend(format!(
            "CEF version {cef_version} not found for {target}: {err}"
        ))
    })?;

    let archive_path = version
        .download_archive_with_retry_from(
            &download_url,
            output_dir,
            show_progress,
            Duration::from_secs(15),
            3,
        )
        .map_err(|err| {
            Error::Backend(format!("CEF archive download failed for {target}: {err}"))
        })?;

    let extracted_dir = download_cef::extract_target_archive(
        target.triple(),
        &archive_path,
        output_dir,
        show_progress,
    )
    .map_err(|err| Error::Backend(format!("CEF archive extraction failed for {target}: {err}")))?;

    version.write_archive_json(&extracted_dir).map_err(|err| {
        Error::Backend(format!("failed to write archive.json for {target}: {err}"))
    })?;

    let _ = std::fs::remove_file(&archive_path);

    Ok(extracted_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_all_returns_all_six_desktop_targets() {
        let targets = EngineTarget::parse_targets("all").unwrap();
        assert_eq!(targets, EngineTarget::ALL);
    }

    #[test]
    fn parse_os_groups_and_deduplicates() {
        let targets = EngineTarget::parse_targets("linux, windows-x64, macos, linux-x64").unwrap();
        assert_eq!(
            targets,
            vec![
                EngineTarget::LinuxX86_64,
                EngineTarget::LinuxAarch64,
                EngineTarget::WindowsX86_64,
                EngineTarget::MacosAarch64,
                EngineTarget::MacosX86_64,
            ]
        );
    }

    #[test]
    fn target_dir_names_match_download_cef_convention() {
        assert_eq!(EngineTarget::LinuxX86_64.dir_name(), "cef_linux_x86_64");
        assert_eq!(EngineTarget::WindowsX86_64.dir_name(), "cef_windows_x86_64");
        assert_eq!(EngineTarget::MacosAarch64.dir_name(), "cef_macos_aarch64");
    }

    #[test]
    fn parse_rejects_unknown_target() {
        assert!(EngineTarget::parse_targets("freebsd").is_err());
        assert!(EngineTarget::parse_targets(" , ").is_err());
    }

    #[test]
    fn sdk_detects_installed_engine_layout() {
        let temp = std::env::temp_dir().join(format!("gpui-cef-sdk-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        let sdk = EngineSdk::new(&temp);
        assert!(!sdk.is_installed(EngineTarget::LinuxX86_64));

        let linux_dir = sdk.target_dir(EngineTarget::LinuxX86_64);
        std::fs::create_dir_all(linux_dir.join("locales")).unwrap();
        std::fs::write(linux_dir.join("icudtl.dat"), b"icu").unwrap();
        std::fs::write(linux_dir.join("resources.pak"), b"pak").unwrap();
        assert!(sdk.is_installed(EngineTarget::LinuxX86_64));

        sdk.uninstall(EngineTarget::LinuxX86_64).unwrap();
        assert!(!sdk.is_installed(EngineTarget::LinuxX86_64));
        let _ = std::fs::remove_dir_all(&temp);
    }
}
