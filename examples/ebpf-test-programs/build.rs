use which::which;

fn main() {
    let bpf_linker = which("bpf-linker").unwrap();
    println!("cargo:rerun-if-changed={}", bpf_linker.to_str().unwrap());

    // waggle::waggle_build();

    // waggle::Builder::new()
    //     .scan("src")
    //     .generate("calls.rs")
    //     .run()
    //     .unwrap();
}
