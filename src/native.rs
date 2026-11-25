use crate::audit::{Receipt, module_bytes};
use goblin::{Object, elf, mach, pe};
use serde::Serialize;
use std::{collections::BTreeSet, error::Error, path::Path};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const REQUIRED: [&str; 3] = [
    "obs_module_load",
    "obs_module_set_pointer",
    "obs_module_ver",
];

#[derive(Serialize)]
pub struct ModuleReceipt {
    #[serde(flatten)]
    archive: Receipt,
    module_path: String,
    format: &'static str,
    architecture: &'static str,
    required_exports: Vec<&'static str>,
    linked_libraries: BTreeSet<String>,
}

pub fn inspect_module(path: &Path, module: &str, target: &str, obs: &str) -> Result<ModuleReceipt> {
    if !module
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphanumeric)
        || !module
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err("Invalid module identifier".into());
    }
    if !matches!(obs, "32.2.0" | "33.0.3") {
        return Err("Unsupported OBS version".into());
    }
    let module_path = match target {
        "linux-x86_64" | "windows-x64" => {
            let ext = if target == "linux-x86_64" {
                "so"
            } else {
                "dll"
            };
            let middle = if obs == "32.2.0" { "bin/64bit/" } else { "" };
            format!("{module}/{middle}{module}.{ext}")
        }
        "macos-arm64" | "macos-x86_64" => format!("{module}.plugin/Contents/MacOS/{module}"),
        _ => return Err("Unsupported native target".into()),
    };
    let archive_ext = if target == "linux-x86_64" {
        ".tar.gz"
    } else {
        ".zip"
    };
    if !path
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|name| name.ends_with(archive_ext))
    {
        return Err("Archive extension does not match target".into());
    }
    let (archive, bytes) = module_bytes(path, &module_path)?;
    let (format, architecture, exports, libraries): (_, _, BTreeSet<String>, BTreeSet<String>) =
        match Object::parse(&bytes)? {
            Object::Elf(binary) if target == "linux-x86_64"
                && binary.is_64 && binary.little_endian
                && binary.header.e_machine == elf::header::EM_X86_64
                && binary.header.e_type == elf::header::ET_DYN
                && binary.interpreter.is_none() => {
                let exports = binary.dynsyms.iter().filter(|s| s.st_shndx != 0
                    && s.is_function() && matches!(s.st_bind(), elf::sym::STB_GLOBAL | elf::sym::STB_WEAK)
                    && matches!(s.st_visibility(), elf::sym::STV_DEFAULT | elf::sym::STV_PROTECTED))
                    .filter_map(|s| binary.dynstrtab.get_at(s.st_name)).map(str::to_owned).collect();
                ("ELF", "x86_64", exports, binary.libraries.iter().map(|s| (*s).to_owned()).collect())
            }
            Object::PE(binary) if target == "windows-x64" && binary.is_lib && binary.is_64
                && binary.header.coff_header.machine == pe::header::COFF_MACHINE_X86_64 => {
                let exports = binary.exports.iter().filter(|e| e.reexport.is_none() && e.rva != 0)
                    .filter_map(|e| e.name).map(str::to_owned).collect();
                ("PE", "x86_64", exports, binary.libraries.iter().map(|s| (*s).to_owned()).collect())
            }
            Object::Mach(mach::Mach::Binary(binary)) if target.starts_with("macos-")
                && binary.is_64 && matches!(binary.header.filetype, mach::header::MH_BUNDLE | mach::header::MH_DYLIB) => {
                let arch = match binary.header.cputype {
                    mach::constants::cputype::CPU_TYPE_ARM64 if target == "macos-arm64" => "arm64",
                    mach::constants::cputype::CPU_TYPE_X86_64 if target == "macos-x86_64" => "x86_64",
                    _ => return Err("Mach-O architecture does not match target".into()),
                };
                let exports = binary.exports()?.into_iter().filter(|e| !matches!(e.info, mach::exports::ExportInfo::Reexport { .. }))
                    .filter_map(|e| e.name.strip_prefix('_').map(str::to_owned)).collect();
                ("Mach-O", arch, exports, binary.libs.iter().filter(|s| **s != "self").map(|s| (*s).to_owned()).collect())
            }
            _ => return Err("Binary format, architecture or library type does not match target (universal Mach-O not supported)".into()),
        };
    let missing: Vec<_> = REQUIRED
        .into_iter()
        .filter(|name| !exports.contains(*name))
        .collect();
    if !missing.is_empty() {
        return Err(format!("Missing OBS entry-point exports: {missing:?}").into());
    }
    Ok(ModuleReceipt {
        archive,
        module_path,
        format,
        architecture,
        required_exports: REQUIRED.to_vec(),
        linked_libraries: libraries,
    })
}
