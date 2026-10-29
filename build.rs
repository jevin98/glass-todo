fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/app.ico");
    println!("cargo:rerun-if-changed=assets/app.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
        let output = out.join("app-resource.o");
        let result = std::process::Command::new(
            std::env::var_os("WINDRES").unwrap_or_else(|| "windres".into()),
        )
        .args(["-I", "assets", "-i", "assets/app.rc", "-O", "coff", "-o"])
        .arg(&output)
        .status()
        .expect("windres is required to embed the Windows manifest and icon");
        assert!(result.success(), "compiling Windows resources failed");
        println!("cargo:rustc-link-arg={}", output.display());
    } else {
        let output = out.join("app.res");
        let result = std::process::Command::new("rc.exe")
            .args(["/nologo", "/I", "assets", "/fo"])
            .arg(&output)
            .arg("assets/app.rc")
            .status()
            .expect("rc.exe is required; use a Visual Studio Developer shell");
        assert!(result.success(), "compiling Windows resources failed");
        println!("cargo:rustc-link-arg={}", output.display());
    }
}
