fn main() {
    // Build the setup wizard frontend (setup-ui/) so its dist/ can be embedded.
    build_setup_ui();

    // Propagate DISCRAKT_VERSION to the main build via cargo:rustc-env
    // This allows the binary to access the version at compile time via env!("DISCRAKT_VERSION")
    println!("cargo:rerun-if-env-changed=DISCRAKT_VERSION");
    if let Ok(version) = std::env::var("DISCRAKT_VERSION") {
        println!("cargo:rustc-env=DISCRAKT_VERSION={}", version);
    }

    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/Discrakt.ico");
        res.set("ProductName", "Discrakt");
        res.set("FileDescription", "Trakt to Discord Rich Presence");
        res.set("LegalCopyright", "Copyright (c) afonsojramos");

        // Set version from CARGO_PKG_VERSION or DISCRAKT_VERSION env var
        // The release workflow sets DISCRAKT_VERSION to the git tag version
        let version = std::env::var("DISCRAKT_VERSION")
            .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());

        // Parse semantic version (major.minor.patch) for Windows VERSIONINFO
        let version_parts: Vec<&str> = version.split('.').collect();
        let major = version_parts
            .first()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0);
        let minor = version_parts
            .get(1)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0);
        let patch = version_parts
            .get(2)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0);

        // Windows VERSIONINFO stores version as a 64-bit value with four 16-bit components:
        // | major (16 bits) | minor (16 bits) | patch (16 bits) | build (16 bits) |
        // Bits:  63-48            47-32             31-16              15-0
        let version_u64 = (major as u64) << 48 | (minor as u64) << 32 | (patch as u64) << 16;
        res.set_version_info(winresource::VersionInfo::PRODUCTVERSION, version_u64);
        res.set_version_info(winresource::VersionInfo::FILEVERSION, version_u64);
        res.set("ProductVersion", &version);
        res.set("FileVersion", &version);

        // Add manifest to declare application capabilities transparently
        // This helps reduce AV false positives by explicitly declaring the app's intent
        res.set_manifest(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity
    version="1.0.0.0"
    processorArchitecture="*"
    name="com.afonsojramos.discrakt"
    type="win32"
  />
  <description>Trakt to Discord Rich Presence</description>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <!-- Windows 10 and Windows 11 -->
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
      <!-- Windows 8.1 -->
      <supportedOS Id="{1f676c76-80e1-4239-95bb-83d0f6d0da78}"/>
      <!-- Windows 8 -->
      <supportedOS Id="{4a2f28e3-53b9-4441-ba9c-d69d4a4a6e38}"/>
      <!-- Windows 7 -->
      <supportedOS Id="{35138b9a-5d96-4fbd-8e2d-a2440225f93a}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">permonitorv2,permonitor</dpiAwareness>
    </windowsSettings>
  </application>
</assembly>"#,
        );

        res.compile()
            .expect("Failed to compile Windows resources. Ensure assets/Discrakt.ico exists");
    }
}

/// Builds the embedded setup-ui frontend (vite-plus) into setup-ui/dist.
///
/// Set `DISCRAKT_SKIP_UI_BUILD=1` to skip this and reuse an existing dist/
/// (e.g. in environments without Node/pnpm that ship a prebuilt bundle).
fn build_setup_ui() {
    // Rebuild only when the frontend sources change.
    for path in [
        "setup-ui/src",
        "setup-ui/index.html",
        "setup-ui/package.json",
        "setup-ui/pnpm-lock.yaml",
        "setup-ui/vite.config.ts",
        "setup-ui/components.json",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    println!("cargo:rerun-if-env-changed=DISCRAKT_SKIP_UI_BUILD");

    if std::env::var_os("DISCRAKT_SKIP_UI_BUILD").is_some() {
        println!("cargo:warning=Skipping setup-ui build; reusing existing setup-ui/dist");
        return;
    }

    let ui_dir = std::path::Path::new("setup-ui");

    // Install dependencies only when they are missing (keeps incremental builds fast).
    if !ui_dir.join("node_modules").exists() {
        run_in("pnpm", &["install", "--frozen-lockfile"], ui_dir);
    }
    run_in("pnpm", &["run", "build"], ui_dir);
}

fn run_in(program: &str, args: &[&str], dir: &std::path::Path) {
    let status = std::process::Command::new(program)
        .args(args)
        .current_dir(dir)
        .status();

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => panic!("`{program} {}` failed with {s}", args.join(" ")),
        Err(e) => panic!(
            "failed to run `{program}`: {e}. Install Node + pnpm, or set \
             DISCRAKT_SKIP_UI_BUILD=1 to reuse a prebuilt setup-ui/dist."
        ),
    }
}
