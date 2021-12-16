//! build.rs for `libafl_targets`

use std::{env, fs::File, io::Write, path::Path};

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_dir = out_dir.to_string_lossy().to_string();
    //let out_dir_path = Path::new(&out_dir);
    let src_dir = Path::new("src");

    let dest_path = Path::new(&out_dir).join("constants.rs");
    let mut constants_file = File::create(&dest_path).expect("Could not create file");

    let edges_map_size: usize = option_env!("LIBAFL_EDGES_MAP_SIZE")
        .map_or(Ok(65536), str::parse)
        .expect("Could not parse LIBAFL_EDGES_MAP_SIZE");
    let cmp_map_size: usize = option_env!("LIBAFL_CMP_MAP_SIZE")
        .map_or(Ok(65536), str::parse)
        .expect("Could not parse LIBAFL_CMP_MAP_SIZE");
    let cmplog_map_w: usize = option_env!("LIBAFL_CMPLOG_MAP_W")
        .map_or(Ok(65536), str::parse)
        .expect("Could not parse LIBAFL_CMPLOG_MAP_W");
    let cmplog_map_h: usize = option_env!("LIBAFL_CMPLOG_MAP_H")
        .map_or(Ok(32), str::parse)
        .expect("Could not parse LIBAFL_CMPLOG_MAP_H");

    write!(
        &mut constants_file,
        "// These constants are autogenerated by build.rs

        /// The size of the edges map
        pub const EDGES_MAP_SIZE: usize = {};
        /// The size of the cmps map
        pub const CMP_MAP_SIZE: usize = {};
        /// The width of the `CmpLog` map
        pub const CMPLOG_MAP_W: usize = {};
        /// The height of the `CmpLog` map
        pub const CMPLOG_MAP_H: usize = {};
",
        edges_map_size, cmp_map_size, cmplog_map_w, cmplog_map_h
    )
    .expect("Could not write file");

    println!("cargo:rerun-if-env-changed=LIBAFL_EDGES_MAP_SIZE");
    println!("cargo:rerun-if-env-changed=LIBAFL_CMP_MAP_SIZE");
    println!("cargo:rerun-if-env-changed=LIBAFL_CMPLOG_MAP_W");
    println!("cargo:rerun-if-env-changed=LIBAFL_CMPLOG_MAP_H");

    //std::env::set_var("CC", "clang");
    //std::env::set_var("CXX", "clang++");

    #[cfg(any(feature = "sancov_value_profile", feature = "sancov_cmplog"))]
    {
        println!("cargo:rerun-if-changed=src/sancov_cmp.c");

        let mut sancov_cmp = cc::Build::new();

        #[cfg(feature = "sancov_value_profile")]
        {
            sancov_cmp.define("SANCOV_VALUE_PROFILE", "1");
            println!("cargo:rerun-if-changed=src/value_profile.h");
        }

        #[cfg(feature = "sancov_cmplog")]
        {
            sancov_cmp.define("SANCOV_CMPLOG", "1");
        }

        sancov_cmp
            .define("CMP_MAP_SIZE", Some(&*format!("{}", cmp_map_size)))
            .define("CMPLOG_MAP_W", Some(&*format!("{}", cmplog_map_w)))
            .define("CMPLOG_MAP_H", Some(&*format!("{}", cmplog_map_h)))
            .file(src_dir.join("sancov_cmp.c"))
            .compile("sancov_cmp");
    }

    #[cfg(feature = "libfuzzer")]
    {
        println!("cargo:rerun-if-changed=src/libfuzzer.c");

        cc::Build::new()
            .file(src_dir.join("libfuzzer.c"))
            .compile("libfuzzer");
    }

    println!("cargo:rerun-if-changed=src/common.h");
    println!("cargo:rerun-if-changed=src/common.c");

    cc::Build::new()
        .file(src_dir.join("common.c"))
        .compile("common");

    println!("cargo:rerun-if-changed=src/coverage.c");

    cc::Build::new()
        .file(src_dir.join("coverage.c"))
        .define("EDGES_MAP_SIZE", Some(&*format!("{}", edges_map_size)))
        .compile("coverage");

    println!("cargo:rerun-if-changed=src/cmplog.h");
    println!("cargo:rerun-if-changed=src/cmplog.c");

    cc::Build::new()
        .define("CMPLOG_MAP_W", Some(&*format!("{}", cmplog_map_w)))
        .define("CMPLOG_MAP_H", Some(&*format!("{}", cmplog_map_h)))
        .file(src_dir.join("cmplog.c"))
        .compile("cmplog");

    println!("cargo:rustc-link-search=native={}", &out_dir);

    println!("cargo:rerun-if-changed=build.rs");
}
