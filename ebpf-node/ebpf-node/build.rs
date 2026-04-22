use std::env;

fn main() {
    // Check that bpf-linker is installed
    if which::which("bpf-linker").is_err() {
        panic!("bpf-linker is required to build this crate. Install it with: cargo install bpf-linker");
    }

    // Build the eBPF program using aya-build
    let ebpf_pkg = aya_build::Package {
        name: "ebpf-node-ebpf",
        root_dir: "../ebpf-node-ebpf",
        no_default_features: false,
        features: &[],
    };

    aya_build::build_ebpf([ebpf_pkg], aya_build::Toolchain::Nightly).expect("failed to build eBPF programs");
}
