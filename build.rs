use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    #[cfg(target_os = "windows")]
    {
        println!("cargo:rerun-if-changed=assets/icon.ico");
        println!("cargo:rerun-if-changed=build.rs");
        println!("cargo:rerun-if-changed=Cargo.toml");

        let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set by Cargo");
        let out = PathBuf::from(&out_dir);

        let icon_path = Path::new("assets/icon.ico");
        let abs_ico = std::fs::canonicalize(icon_path)
            .unwrap_or_else(|_| icon_path.to_path_buf());
        let abs_ico_str = abs_ico.to_string_lossy().replace('\\', "/");
        let clean_ico_path = abs_ico_str.strip_prefix("//?/").unwrap_or(&abs_ico_str);

        let icon_bytes = include_bytes!("assets/icon.ico");
        if let Ok(img) = image::load_from_memory(icon_bytes) {
            let resized = img.resize_exact(32, 32, image::imageops::FilterType::Lanczos3);
            let rgba = resized.to_rgba8().into_raw();
            let _ = std::fs::write(out.join("icon_32.rgba"), rgba);
        }

        let pkg_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "1.1.0".to_string());
        let mut version_parts = pkg_version.split('.');
        let major: u32 = version_parts.next().and_then(|v| v.parse().ok()).unwrap_or(1);
        let minor: u32 = version_parts.next().and_then(|v| v.parse().ok()).unwrap_or(1);
        let patch: u32 = version_parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);

        let rc_content = format!(
            r#"#pragma code_page(65001)
1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "041604b0"
        BEGIN
            VALUE "CompanyName", "Leandro Pinheiro"
            VALUE "FileDescription", "DuckDNS Updater - Dynamic DNS Client"
            VALUE "FileVersion", "{pkg_version}.0"
            VALUE "InternalName", "duckdns-updater"
            VALUE "LegalCopyright", "Copyright © 2026 Leandro Pinheiro. MIT License."
            VALUE "OriginalFilename", "duckdns-updater.exe"
            VALUE "ProductName", "DuckDNS Updater"
            VALUE "ProductVersion", "{pkg_version}.0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x416, 1200
    END
END
1 ICON "{clean_ico_path}"
"#
        );

        let generated_rc_path = out.join("app_resource.rc");
        let _ = std::fs::write(&generated_rc_path, &rc_content);

        // Step 1: compile .rc → .res (Windows binary resource format, not COFF)
        let res_path = out.join("windows.res");
        let rc_ok = try_windres_to_res(generated_rc_path.to_str().unwrap(), res_path.to_str().unwrap());

        if rc_ok {
            // Step 2: convert .res → COFF .o via llvm-cvtres, which lld can link.
            let obj_path = out.join("windows_resource.o");
            let cvtres_ok = Command::new("llvm-cvtres")
                .args([
                    res_path.to_str().unwrap(),
                    &format!("/OUT:{}", obj_path.to_str().unwrap()),
                    "/MACHINE:X64",
                    "/NOLOGO",
                ])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

            if cvtres_ok {
                // Link the resource object into the final binary.
                println!(
                    "cargo:rustc-link-arg={}",
                    obj_path.to_str().unwrap()
                );
                return;
            }

            // cvtres failed: try passing the raw COFF .res directly
            let coff_path = out.join("windows_coff.res");
            if try_windres_to_coff(generated_rc_path.to_str().unwrap(), coff_path.to_str().unwrap()) {
                println!(
                    "cargo:rustc-link-arg={}",
                    coff_path.to_str().unwrap()
                );
                return;
            }
        }

        // Fallback for MSVC / rc.exe
        embed_resource::compile(generated_rc_path.to_str().unwrap(), embed_resource::NONE);
    }
}

/// Compile an .rc file to Windows binary .res format via llvm-windres or windres.
fn try_windres_to_res(rc: &str, out: &str) -> bool {
    let winlibs_windres = r"C:\Users\Leandro\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin\windres.exe";
    let candidates = [
        "llvm-windres",
        "windres",
        winlibs_windres,
        "x86_64-w64-mingw32-windres",
    ];

    for tool in &candidates {
        if Command::new(tool)
            .args([
                "--input", rc,
                "--output", out,
                "--output-format=res",
                "--target=pe-x86-64",
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

/// Compile an .rc file to COFF format via llvm-windres or windres.
fn try_windres_to_coff(rc: &str, out: &str) -> bool {
    let winlibs_windres = r"C:\Users\Leandro\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin\windres.exe";
    let candidates = [
        "llvm-windres",
        "windres",
        winlibs_windres,
        "x86_64-w64-mingw32-windres",
    ];

    for tool in &candidates {
        if Command::new(tool)
            .args([
                "--input", rc,
                "--output", out,
                "--output-format=coff",
                "--target=pe-x86-64",
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}
