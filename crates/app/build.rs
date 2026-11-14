fn main() {
    println!("cargo:rerun-if-changed=lang");
    println!("cargo:rerun-if-changed=assets");

    // Icon, full version information and a standard manifest for
    // Batiie.exe. Antivirus programs trust a program more when it says
    // clearly who made it, what it is and that it runs without admin rights.
    #[cfg(windows)]
    {
        let version = env!("CARGO_PKG_VERSION");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icon.ico");
        res.set("ProductName", "Batiie");
        res.set("FileDescription", "Batiie");
        res.set("CompanyName", "Batiie");
        res.set("LegalCopyright", "Copyright (C) 2026 Batiie Team");
        res.set("OriginalFilename", "Batiie.exe");
        res.set("InternalName", "Batiie");
        res.set("ProductVersion", version);
        res.set("FileVersion", version);
        res.set_manifest(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="Batiie.Batiie" version="1.0.0.0" processorArchitecture="*"/>
  <description>Batiie - download manager</description>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
</assembly>
"#,
        );
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed the Windows icon: {e}");
        }
    }
}
