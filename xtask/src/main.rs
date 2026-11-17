const EXECUTABLE: &str = "gpui-cef-browser";

fn main() -> anyhow::Result<()> {
    let app = bundle()?;
    if std::env::args().nth(1).as_deref() == Some("run") {
        launch(app)?;
    } else {
        println!("{}", app.display());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn launch(app: std::path::PathBuf) -> anyhow::Result<()> {
    let status = std::process::Command::new("open").arg(app).status()?;
    check_status(status)
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn launch(app: std::path::PathBuf) -> anyhow::Result<()> {
    check_status(std::process::Command::new(app).status()?)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn launch(_app: std::path::PathBuf) -> anyhow::Result<()> {
    anyhow::bail!("CEF bundles are only supported on macOS, Linux, and Windows")
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn check_status(status: std::process::ExitStatus) -> anyhow::Result<()> {
    if !status.success() {
        anyhow::bail!("browser process exited with {status}");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn bundle() -> anyhow::Result<std::path::PathBuf> {
    use cef::build_util::mac::{build_bundle, BundleInfo};

    let out_dir = std::path::PathBuf::from("target/bundle");
    std::fs::create_dir_all(&out_dir)?;

    build_bundle(
        &out_dir,
        EXECUTABLE,
        BundleInfo::new(
            EXECUTABLE,
            "dev.grovudev.gpui-cef-browser",
            "gpui-cef browser",
            "en",
            semver::Version::new(0, 2, 0),
        ),
    )
    .map_err(|err| anyhow::anyhow!("failed to bundle CEF: {err:?}"))
}

#[cfg(target_os = "linux")]
fn bundle() -> anyhow::Result<std::path::PathBuf> {
    let out_dir = std::path::PathBuf::from("target/bundle");
    std::fs::create_dir_all(&out_dir)?;

    cef::build_util::linux::build_bundle(&out_dir, EXECUTABLE, false)
        .map_err(|err| anyhow::anyhow!("failed to bundle CEF: {err:?}"))?;

    let launcher = out_dir.join("run.sh");
    std::fs::write(
        &launcher,
        format!(
            "#!/bin/sh\nset -eu\nAPP_DIR=$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\n\
             export LD_LIBRARY_PATH=\"$APP_DIR${{LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}}\"\n\
             exec \"$APP_DIR/{EXECUTABLE}\" \"$@\"\n"
        ),
    )?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755))?;
    Ok(launcher)
}

#[cfg(target_os = "windows")]
fn bundle() -> anyhow::Result<std::path::PathBuf> {
    let out_dir = std::path::PathBuf::from("target/bundle");
    std::fs::create_dir_all(&out_dir)?;

    cef::build_util::win::build_bundle(&out_dir, EXECUTABLE, false)
        .map_err(|err| anyhow::anyhow!("failed to bundle CEF: {err:?}"))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn bundle() -> anyhow::Result<std::path::PathBuf> {
    anyhow::bail!("CEF bundles are only supported on macOS, Linux, and Windows")
}
