//! With the `extension-module` feature the CPython symbols stay unresolved until the
//! interpreter loads the module; macOS needs `-undefined dynamic_lookup` for that link to
//! succeed. maturin passes the flag itself — this makes plain `cargo build` work too.
fn main() {
    pyo3_build_config::add_extension_module_link_args();
}
