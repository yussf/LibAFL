use libafl_cc::{ClangWrapper, CompilerWrapper};
use std::env;

pub fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        let fuzzlib = env::var("LIBAFL_FUZZLIB")
            .expect("You need to set the LIBAFL_FUZZLIB env var to the path of the fuzzer lib");
        let mut dir = env::current_exe().unwrap();
        let wrapper_name = dir.file_name().unwrap().to_str().unwrap();

        let is_cpp = match wrapper_name[wrapper_name.len()-2..].to_lowercase().as_str() {
            "cc" => false,
            "++" | "pp" | "xx" => true,
            _ => panic!("Could not figure out if c or c++ warpper was called. Expected {:?} to end with c or cxx", dir),
        };

        dir.pop();

        let mut cc = ClangWrapper::new();
        if let Some(code) = cc
            .wrapped_cc(env::var("LIBAFL_CC").expect("You need to set the LIBAFL_CC env var"))
            .wrapped_cxx(env::var("LIBAFL_CXX").expect("You need to set the LIBAFL_CXX env var"))
            .cpp(is_cpp)
            // silence the compiler wrapper output, needed for some configure scripts.
            .silence(true)
            .from_args(&args)
            .expect("Failed to parse the command line")
            .add_arg("-fsanitize-coverage=trace-pc-guard")
            .add_link_arg("-Wl,--whole-archive")
            .add_link_arg(fuzzlib)
            .add_link_arg("-Wl,-no-whole-archive")
            .run()
            .expect("Failed to run the wrapped compiler")
        {
            std::process::exit(code);
        }
    } else {
        panic!("LibAFL CC: No Arguments given");
    }
}
