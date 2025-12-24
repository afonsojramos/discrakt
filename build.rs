fn main() {
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

        res.compile()
            .expect("Failed to compile Windows resources. Ensure assets/Discrakt.ico exists");
    }
}
