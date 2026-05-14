// Forge build script — compiles cuda/kasm_interpret.cu into a fatbin
// containing PRE-COMPILED CUBINS (SASS code) for every NVIDIA GPU
// architecture from Turing (2018) onwards. The driver loads the matching
// cubin directly — NO PTX JIT, no driver-version dependency, no PTX
// version mismatch. The user just needs nvcuda.dll (ships with every
// NVIDIA driver since 2007).
//
// Why fatbin and not PTX:
//   PTX version is pinned to nvcc's release. nvcc 13.2 emits PTX 9.2.
//   A user's driver supporting only PTX 9.1 (e.g. CUDA 13.1 driver) can't
//   JIT it — error: "unsupported toolchain". Cubins skip the JIT entirely
//   because they're already SASS for a specific architecture.
//
// Architectures embedded:
//   sm_75 (Turing 2018+) — GTX 1660, RTX 20-series, T4
//   sm_80 (Ampere data center) — A100, A30
//   sm_86 (Ampere consumer) — RTX 30-series, A40, A2
//   sm_89 (Ada Lovelace) — RTX 40-series, L40
//   sm_90 (Hopper) — H100, H200
//   compute_90 PTX fallback for any future GPU not in this list
//
// Skip silently when the cuda feature is off OR nvcc is not on PATH —
// the build still succeeds without GPU support, the binary just won't
// have CUDA acceleration.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=cuda/kasm_interpret.cu");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CUDA_PATH");

    // Always emit a PTX file in OUT_DIR so include_str! at runtime
    // doesn't fail the build. If we can't compile, write an empty
    // placeholder — the runtime detects empty PTX and skips CUDA.
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR set by cargo"));
    let fatbin_path = out_dir.join("kasm.fatbin");

    if env::var("CARGO_FEATURE_CUDA").is_err() {
        // Feature disabled — write empty placeholder.
        std::fs::write(&fatbin_path, []).expect("write empty fatbin placeholder");
        return;
    }

    let nvcc = find_nvcc();
    let nvcc = match nvcc {
        Some(p) => p,
        None => {
            // No nvcc on the build machine. Write empty placeholder so
            // the include_str! resolves to an empty string and the
            // runtime knows the kernel isn't available.
            println!(
                "cargo:warning=Forge: nvcc not found on PATH or under CUDA_PATH/CUDA_HOME — \
                 building without embedded CUDA kernel. Install CUDA Toolkit on the build \
                 machine (not the user's) to enable GPU acceleration."
            );
            std::fs::write(&fatbin_path, []).expect("write empty fatbin placeholder");
            return;
        }
    };

    let src = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("cuda")
        .join("kasm_interpret.cu");
    if !src.exists() {
        println!(
            "cargo:warning=Forge: cuda/kasm_interpret.cu not found at {} — skipping fatbin build",
            src.display()
        );
        std::fs::write(&fatbin_path, []).expect("write empty fatbin placeholder");
        return;
    }

    // Compile CUDA C → fatbin (universal binary containing one cubin per
    // architecture). The driver loads the matching cubin DIRECTLY — no
    // PTX JIT, no PTX version dependency. We also include a single PTX
    // entry as forward-compat fallback for unknown future GPUs.
    //
    // Architecture coverage (every consumer + datacenter NVIDIA since 2018):
    //   sm_75 = Turing       (RTX 20xx, GTX 16xx, T4, Quadro RTX)
    //   sm_80 = Ampere DC    (A100, A30)
    //   sm_86 = Ampere       (RTX 30xx, A40, A2)
    //   sm_89 = Ada Lovelace (RTX 40xx, L40, RTX 6000 Ada)
    //   sm_90 = Hopper       (H100, H200)
    //   compute_90 PTX       (forward-compat fallback for sm_100+ Blackwell etc)
    //
    // On Windows nvcc needs cl.exe (MSVC host compiler). Rust toolchain
    // already requires MSVC to link, so it's always present — just not
    // always on PATH. We detect it and pass via -ccbin.
    let mut cmd = Command::new(&nvcc);
    cmd.arg("-fatbin");
    cmd.arg("-gencode").arg("arch=compute_75,code=sm_75");
    cmd.arg("-gencode").arg("arch=compute_80,code=sm_80");
    cmd.arg("-gencode").arg("arch=compute_86,code=sm_86");
    cmd.arg("-gencode").arg("arch=compute_89,code=sm_89");
    cmd.arg("-gencode").arg("arch=compute_90,code=sm_90");
    cmd.arg("-gencode").arg("arch=compute_90,code=compute_90"); // PTX fallback
    cmd.arg("-o").arg(&fatbin_path);
    #[cfg(windows)]
    {
        if let Some(cl_dir) = find_cl_exe_dir() {
            cmd.arg("-ccbin").arg(&cl_dir);
            let existing = env::var("PATH").unwrap_or_default();
            let new_path = format!("{};{}", cl_dir.display(), existing);
            cmd.env("PATH", new_path);
        } else {
            println!(
                "cargo:warning=Forge: cl.exe (MSVC) not found — nvcc will fail. \
                 Install Visual Studio Build Tools or rebuild from a Developer Command Prompt."
            );
        }
    }
    cmd.arg(&src);
    let status = cmd.status();

    match status {
        Ok(s) if s.success() => {
            let size = std::fs::metadata(&fatbin_path)
                .map(|m| m.len())
                .unwrap_or(0);
            println!(
                "cargo:warning=Forge: kasm_interpret.cu compiled to fatbin ({} KB, sm_75/80/86/89/90 + compute_90 PTX) at {}",
                size / 1024,
                fatbin_path.display()
            );
        }
        Ok(s) => {
            println!(
                "cargo:warning=Forge: nvcc exited with status {} — falling back to empty fatbin",
                s
            );
            std::fs::write(&fatbin_path, []).expect("write empty fatbin placeholder");
        }
        Err(e) => {
            println!(
                "cargo:warning=Forge: nvcc invocation failed ({}) — falling back to empty fatbin",
                e
            );
            std::fs::write(&fatbin_path, []).expect("write empty fatbin placeholder");
        }
    }
}

#[cfg(windows)]
fn find_cl_exe_dir() -> Option<PathBuf> {
    // 1. Already on PATH? (developer command prompt case)
    if let Ok(path_var) = env::var("PATH") {
        for dir in path_var.split(';') {
            if dir.is_empty() {
                continue;
            }
            let p = PathBuf::from(dir).join("cl.exe");
            if p.exists() {
                return Some(p.parent()?.to_path_buf());
            }
        }
    }
    // 2. VS 2022 / 2019 / 2017 Community / Professional / Enterprise.
    const VS_ROOTS: &[&str] = &[
        r"C:\Program Files\Microsoft Visual Studio",
        r"C:\Program Files (x86)\Microsoft Visual Studio",
    ];
    const EDITIONS: &[&str] = &["Community", "Professional", "Enterprise", "BuildTools"];
    for root in VS_ROOTS {
        let root_path = PathBuf::from(root);
        if !root_path.exists() {
            continue;
        }
        // Year subdirs: 2022, 2019, 2017 ...
        if let Ok(years) = std::fs::read_dir(&root_path) {
            let mut year_dirs: Vec<PathBuf> =
                years.flatten().map(|e| e.path()).collect();
            year_dirs.sort();
            for year_dir in year_dirs.into_iter().rev() {
                for ed in EDITIONS {
                    let msvc_root = year_dir.join(ed).join(r"VC\Tools\MSVC");
                    if let Ok(versions) = std::fs::read_dir(&msvc_root) {
                        let mut version_dirs: Vec<PathBuf> =
                            versions.flatten().map(|e| e.path()).collect();
                        version_dirs.sort();
                        for ver_dir in version_dirs.into_iter().rev() {
                            let candidate =
                                ver_dir.join(r"bin\Hostx64\x64\cl.exe");
                            if candidate.exists() {
                                return Some(candidate.parent()?.to_path_buf());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn find_nvcc() -> Option<PathBuf> {
    // 1. PATH lookup via OS resolution.
    let cmd = if cfg!(windows) { "nvcc.exe" } else { "nvcc" };
    if let Ok(path_var) = env::var("PATH") {
        let sep = if cfg!(windows) { ';' } else { ':' };
        for dir in path_var.split(sep) {
            if dir.is_empty() {
                continue;
            }
            let candidate = PathBuf::from(dir).join(cmd);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // 2. CUDA_PATH / CUDA_HOME env var → bin/<cmd>.
    for var in &["CUDA_PATH", "CUDA_HOME"] {
        if let Ok(root) = env::var(var) {
            let candidate = PathBuf::from(&root).join("bin").join(cmd);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // 3. Standard Windows install location, scan version dirs.
    #[cfg(windows)]
    {
        const TOOLKIT_ROOT: &str = r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA";
        if let Ok(entries) = std::fs::read_dir(TOOLKIT_ROOT) {
            let mut versions: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            versions.sort();
            for path in versions.into_iter().rev() {
                let candidate = path.join("bin").join(cmd);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }

    // 4. Standard Linux install.
    #[cfg(unix)]
    {
        const STANDARD_PATHS: &[&str] = &[
            "/usr/local/cuda/bin/nvcc",
            "/opt/cuda/bin/nvcc",
        ];
        for p in STANDARD_PATHS {
            let path = PathBuf::from(p);
            if path.exists() {
                return Some(path);
            }
        }
    }

    None
}
