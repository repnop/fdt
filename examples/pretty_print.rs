static MY_FDT: &[u8] = include_bytes!("../dtb/test.dtb");

fn main() {
    let fdt = fdt::Fdt::new_unaligned(MY_FDT).unwrap();
    println!("{fdt}");
}
