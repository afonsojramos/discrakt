fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/Discrakt.ico");
        res.set("ProductName", "Discrakt");
        res.set("FileDescription", "Trakt to Discord Rich Presence");
        res.set("LegalCopyright", "Copyright (c) 2024 afonsojramos");
        res.compile().expect("Failed to compile Windows resources");
    }
}
