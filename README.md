# `fdt`

A pure-Rust crate for parsing Flattened Devicetrees, with the goal of having a
very ergonomic and idiomatic API.

[![crates.io](https://img.shields.io/crates/v/fdt.svg)](https://crates.io/crates/fdt) [![Documentation](https://docs.rs/fdt/badge.svg)](https://docs.rs/fdt) ![Build](https://github.com/repnop/fdt/actions/workflows/test.yml/badge.svg?branch=master&event=push)

## License

This crate is licensed under the Mozilla Public License 2.0 (see the LICENSE file).

## Example

```rust
static MY_FDT: &[u8] = include_bytes!("my_fdt.dtb");

fn main() {
    let fdt = fdt::Fdt::new(MY_FDT).unwrap();

    println!("This is a devicetree representation of a {}", fdt.root().model());
    println!("...Which is compatible with: {}", fdt.root().compatible().join(","));
}
```