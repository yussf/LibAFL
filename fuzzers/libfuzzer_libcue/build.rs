// build.rs

use std::{
    env,
    path::Path,
    process::{exit, Command},
};

const LIBCUE_URL: &str = "https://github.com/lipnitsk/libcue/archive/v2.2.1.tar.gz";

fn main() {
    if cfg!(windows) {
        println!("cargo:warning=Skipping libcue example on Windows");
        exit(0);
    }

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let cwd = env::current_dir().unwrap().to_string_lossy().to_string();
    let out_dir = out_dir.to_string_lossy().to_string();
    let out_dir_path = Path::new(&out_dir);

    println!("cargo:rerun-if-changed=./runtime/rt.c",);
    println!("cargo:rerun-if-changed=harness.cc");

    let libcue = format!("{}/libcue-2.2.1", &out_dir);
    let libcue_path = Path::new(&libcue);
    let libcue_tar = format!("{}/v2.2.1.tar.gz", &out_dir);
    let libcue_patch = format!("{}/cue_fixes.patch", &cwd);

    // Enforce clang for its -fsanitize-coverage support.
    std::env::set_var("CC", "clang");
    std::env::set_var("CXX", "clang++");

    if !libcue_path.is_dir() {
        if !Path::new(&libcue_tar).is_file() {
            println!("cargo:warning=libcue not found, downloading...");
            // Download libmozjpeg
            Command::new("wget")
                .arg("-c")
                .arg(LIBCUE_URL)
                .arg("-O")
                .arg(&libcue_tar)
                .status()
                .unwrap();
        }
        Command::new("tar")
             .current_dir(&out_dir_path)
             .arg("-xvf")
             .arg(&libcue_tar)
             .status()
             .unwrap();

    }

   Command::new("patch")
        .current_dir(&libcue)
        .arg("--forward")
        .arg("-p1")
        .arg("-i")
        .arg(&libcue_patch)
        .status()
        .unwrap();

    //println!("cargo:warning={}", format!("{}",  String::from_utf8_lossy(&output.stderr).replace("\n", "")));

    Command::new("cmake")
        .current_dir(&out_dir_path)
        .args(&[
            "-G",
            "Unix Makefiles",
            "--disable-shared",
            &libcue,
        ])
        .env("CC", "clang")
        .env("CXX", "clang++")
        .env(
            "CFLAGS",
            "-O3 -g -D_DEFAULT_SOURCE -fPIE -fsanitize-coverage=trace-pc-guard",
        )
        .env(
            "CXXFLAGS",
            "-O3 -g -D_DEFAULT_SOURCE -fPIE -fsanitize-coverage=trace-pc-guard",
        )
        .env("LDFLAGS", "-g -fPIE -fsanitize-coverage=trace-pc-guard")
        .status()
        .unwrap();

    Command::new("make")
        .current_dir(&out_dir)
        //.arg(&format!("-j{}", num_cpus::get()))
        .env("CC", "clang")
        .env("CXX", "clang++")
        .env(
            "CFLAGS",
            "-O3 -g -D_DEFAULT_SOURCE -fPIE -fsanitize-coverage=trace-pc-guard",
        )
        .env(
            "CXXFLAGS",
            "-O3 -g -D_DEFAULT_SOURCE -fPIE -fsanitize-coverage=trace-pc-guard",
        )
        .env("LDFLAGS", "-g -fPIE -fsanitize-coverage=trace-pc-guard")
        .status()
        .unwrap();

    cc::Build::new()
        .file("../libfuzzer_runtime/rt.c")
        .compile("libfuzzer-sys");

    cc::Build::new()
        .include(&libcue_path)
        .flag("-fsanitize-coverage=trace-pc-guard")
        .file("./harness.cc")
        .compile("libfuzzer-harness");

    println!("cargo:rustc-link-search=native={}", &out_dir);
    println!("cargo:rustc-link-lib=static=cue");

    //For the C++ harness
    println!("cargo:rustc-link-lib=static=stdc++");

    println!("cargo:rerun-if-changed=build.rs");
}
