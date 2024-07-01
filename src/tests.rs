// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

extern crate std;

use nodes::NodeName;
use properties::CellSizes;

// use crate::{node::RawReg, *};
use crate::*;

struct AlignArrayUp<const N: usize>([u8; N]);

impl<const N: usize> AlignArrayUp<N> {
    const fn align_up<const M: usize>(self) -> [u8; M] {
        assert!(M > N);
        assert!(M % 4 == 0);

        let mut copy: [u8; M] = [0u8; M];
        let mut i = 0;

        while i < N {
            copy[i] = self.0[i];
            i += 1;
        }

        copy
    }
}

#[repr(align(4))]
struct Align4<const N: usize>([u8; N]);

impl<const N: usize> Align4<N> {
    const fn new(a: [u8; N]) -> Self {
        Self(a)
    }

    fn as_slice(&self) -> &[u32] {
        unsafe { core::slice::from_raw_parts(self.0.as_ptr().cast::<u32>(), self.0.len() / 4) }
    }
}

static TEST: Align4<3764> =
    Align4::new(AlignArrayUp(*include_bytes!("../dtb/test.dtb")).align_up::<3764>());
static ISSUE_3: Align4<4658> = Align4::new(*include_bytes!("../dtb/issue-3.dtb"));
static SIFIVE: Align4<3872> = Align4::new(*include_bytes!("../dtb/sifive.dtb"));

#[test]
fn returns_fdt() {
    assert!(Fdt::new(TEST.as_slice()).is_ok());
}

#[test]
fn root() {
    let fdt = Fdt::new(TEST.as_slice()).unwrap();
    std::println!("{:?}", fdt.root());
}

#[test]
fn all_nodes() {
    let fdt = Fdt::new(TEST.as_slice()).unwrap();
    assert_eq!(
        fdt.root()
            .all_nodes()
            .map(|(depth, n)| std::format!("{depth} {}", n.name()))
            .collect::<std::vec::Vec<_>>()
            .join("\n"),
        "1 chosen
1 memory@80000000
1 cpus
2 cpu@0
3 interrupt-controller
2 cpu-map
3 cluster0
4 core0
1 emptyproptest
1 soc
2 flash@20000000
2 rtc@101000
2 uart@10000000
2 poweroff
2 reboot
2 test@100000
2 pci@30000000
2 virtio_mmio@10008000
2 virtio_mmio@10007000
2 virtio_mmio@10006000
2 virtio_mmio@10005000
2 virtio_mmio@10004000
2 virtio_mmio@10003000
2 virtio_mmio@10002000
2 virtio_mmio@10001000
2 plic@c000000
2 clint@2000000"
    );
}

#[test]
fn finds_root_node() {
    let fdt = Fdt::new(TEST.as_slice()).unwrap();
    assert!(fdt.root().find_node("/").is_some(), "couldn't find root node");
}

#[test]
fn finds_root_node_properties() {
    // infallible
    let fdt = Fdt::new(TEST.as_slice()).unwrap();
    let prop = fdt.root().find_node("/").unwrap().properties().find("compatible").unwrap();

    assert_eq!(prop.value(), b"riscv-virtio\0");

    // fallible
    let fdt = Fdt::new_fallible(TEST.as_slice()).unwrap();
    let prop = fdt
        .root()
        .unwrap()
        .find_node("/")
        .unwrap()
        .unwrap()
        .properties()
        .unwrap()
        .find("compatible")
        .unwrap()
        .unwrap();

    assert_eq!(prop.value(), b"riscv-virtio\0");
}

#[test]
fn finds_child_of_root_node() {
    let fdt = Fdt::new(TEST.as_slice()).unwrap();
    let root = fdt.root();
    assert_eq!(
        root.find_node("/cpus").unwrap().name(),
        NodeName { name: "cpus", unit_address: None },
        "couldn't find cpus node"
    );

    assert_eq!(
        root.find_node("/cpus/cpu@0/interrupt-controller").unwrap().name(),
        NodeName { name: "interrupt-controller", unit_address: None },
        "couldn't find interrupt-controller node"
    );

    assert!(
        root.find_node("/cpus/cpu@1/interrupt-controller").is_none(),
        "couldn't find interrupt-controller node"
    );
}

#[test]
fn finds_child_with_unit_address() {
    let fdt = Fdt::new(TEST.as_slice()).unwrap();
    let root = fdt.root();
    assert_eq!(
        root.find_node("/memory@80000000").unwrap().name(),
        NodeName { name: "memory", unit_address: Some("80000000") },
        "couldn't find cpus node"
    );
    assert!(root.find_node("/memory@80000001").is_none(), "didn't use unit address to filter!");
}

#[test]
fn properties() {
    let fdt = Fdt::new(TEST.as_slice()).unwrap();
    let test = fdt.root().find_node("/soc/test").unwrap();

    let props =
        test.properties().into_iter().map(|p| (p.name(), p.value())).collect::<std::vec::Vec<_>>();

    assert_eq!(
        props,
        &[
            ("phandle", &[0, 0, 0, 4][..]),
            ("reg", &[0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 0, 16, 0]),
            ("compatible", b"sifive,test1\0sifive,test0\0syscon\0"),
        ]
    );
}

// #[test]
// fn correct_flash_regions() {
//     let fdt = Fdt::new(TEST.as_slice()).unwrap();
//     let regions = fdt.find_node("/soc/flash").unwrap().reg().unwrap().collect::<std::vec::Vec<_>>();

//     assert_eq!(
//         regions,
//         &[
//             MemoryRegion { starting_address: 0x20000000 as *const u8, size: Some(0x2000000) },
//             MemoryRegion { starting_address: 0x22000000 as *const u8, size: Some(0x2000000) }
//         ]
//     );
// }

// #[test]
// fn parses_populated_ranges() {
//     let fdt = Fdt::new(TEST.as_slice()).unwrap();
//     let ranges = fdt.find_node("/soc/pci").unwrap().ranges().unwrap().collect::<std::vec::Vec<_>>();

//     assert_eq!(
//         ranges,
//         &[
//             MemoryRange {
//                 child_bus_address: 0x0000_0000_0000_0000,
//                 child_bus_address_hi: 0x0100_0000,
//                 parent_bus_address: 0x3000000,
//                 size: 0x10000,
//             },
//             MemoryRange {
//                 child_bus_address: 0x40000000,
//                 child_bus_address_hi: 0x2000000,
//                 parent_bus_address: 0x4000_0000,
//                 size: 0x4000_0000,
//             }
//         ]
//     );
// }

// #[test]
// fn parses_empty_ranges() {
//     let fdt = Fdt::new(TEST.as_slice()).unwrap();
//     let ranges = fdt.find_node("/soc").unwrap().ranges().unwrap().collect::<std::vec::Vec<_>>();

//     assert_eq!(ranges, &[]);
// }

// #[test]
// fn finds_with_addr() {
//     let fdt = Fdt::new(TEST.as_slice()).unwrap();
//     assert_eq!(fdt.find_node("/soc/virtio_mmio@10004000").unwrap().name, "virtio_mmio@10004000");
// }

// #[test]
// fn compatibles() {
//     let fdt = Fdt::new(TEST.as_slice()).unwrap();
//     let res = fdt
//         .find_node("/soc/test")
//         .unwrap()
//         .compatible()
//         .unwrap()
//         .all()
//         .all(|s| ["sifive,test1", "sifive,test0", "syscon"].contains(&s));

//     assert!(res);
// }

#[test]
fn cell_sizes() {
    let fdt = Fdt::new(TEST.as_slice()).unwrap();

    let cpu_cs = fdt.root().find_node("/cpus").unwrap().property::<CellSizes>().unwrap();
    assert_eq!(cpu_cs, CellSizes { address_cells: 1, size_cells: 0 });

    let soc_sc = fdt.root().find_node("/soc").unwrap().property::<CellSizes>().unwrap();
    let test_cs = fdt.root().find_node("/soc/test").unwrap().property::<CellSizes>();
    let pci_cs = fdt.root().find_node("/soc/pci").unwrap().property::<CellSizes>().unwrap();
    assert_eq!(soc_sc, CellSizes { address_cells: 2, size_cells: 2 });
    assert_eq!(test_cs, None);
    assert_ne!(pci_cs, soc_sc);
}

// #[test]
// fn no_properties() {
//     let fdt = Fdt::new(TEST.as_slice()).unwrap();
//     let regions = fdt.find_node("/emptyproptest").unwrap();
//     assert_eq!(regions.properties().count(), 0);
// }

// #[test]
// fn finds_all_nodes() {
//     let fdt = Fdt::new(TEST.as_slice()).unwrap();

//     let mut all_nodes: std::vec::Vec<_> = fdt.all_nodes().map(|n| n.name).collect();
//     all_nodes.sort_unstable();

//     assert_eq!(
//         all_nodes,
//         &[
//             "/",
//             "chosen",
//             "clint@2000000",
//             "cluster0",
//             "core0",
//             "cpu-map",
//             "cpu@0",
//             "cpus",
//             "emptyproptest",
//             "flash@20000000",
//             "interrupt-controller",
//             "memory@80000000",
//             "pci@30000000",
//             "plic@c000000",
//             "poweroff",
//             "reboot",
//             "rtc@101000",
//             "soc",
//             "test@100000",
//             "uart@10000000",
//             "virtio_mmio@10001000",
//             "virtio_mmio@10002000",
//             "virtio_mmio@10003000",
//             "virtio_mmio@10004000",
//             "virtio_mmio@10005000",
//             "virtio_mmio@10006000",
//             "virtio_mmio@10007000",
//             "virtio_mmio@10008000"
//         ]
//     )
// }

// #[test]
// fn required_nodes() {
//     let fdt = Fdt::new(TEST.as_slice()).unwrap();
//     fdt.cpus().next().unwrap();
//     fdt.memory();
//     fdt.chosen();
// }

// #[test]
// fn doesnt_exist() {
//     let fdt = Fdt::new(TEST.as_slice()).unwrap();
//     assert!(fdt.find_node("/this/doesnt/exist").is_none());
// }

// #[test]
// fn raw_reg() {
//     let fdt = Fdt::new(TEST.as_slice()).unwrap();
//     let regions =
//         fdt.find_node("/soc/flash").unwrap().raw_reg().unwrap().collect::<std::vec::Vec<_>>();

//     assert_eq!(
//         regions,
//         &[
//             RawReg { address: &0x20000000u64.to_be_bytes(), size: &0x2000000u64.to_be_bytes() },
//             RawReg { address: &0x22000000u64.to_be_bytes(), size: &0x2000000u64.to_be_bytes() }
//         ]
//     );
// }

// #[test]
// fn issue_3() {
//     let fdt = Fdt::new(ISSUE_3.as_slice()).unwrap();
//     fdt.find_all_nodes("uart").for_each(|n| std::println!("{:?}", n));
// }

// #[test]
// fn issue_4() {
//     let fdt = Fdt::new(ISSUE_3.as_slice()).unwrap();
//     fdt.all_nodes().for_each(|n| std::println!("{:?}", n));
// }

// #[test]
// fn cpus() {
//     let fdt = Fdt::new(TEST.as_slice()).unwrap();
//     for cpu in fdt.cpus() {
//         cpu.ids().all().for_each(|n| std::println!("{:?}", n));
//     }
// }

// #[test]
// fn invalid_node() {
//     let fdt = Fdt::new(TEST.as_slice()).unwrap();
//     assert!(fdt.find_node("this/is/an invalid node///////////").is_none());
// }

// #[test]
// fn aliases() {
//     let fdt = Fdt::new(SIFIVE.as_slice()).unwrap();
//     let aliases = fdt.aliases().unwrap();
//     for (_, node_path) in aliases.all() {
//         assert!(fdt.find_node(node_path).is_some(), "path: {:?}", node_path);
//     }
// }

// #[test]
// fn stdout() {
//     let fdt = Fdt::new(TEST.as_slice()).unwrap();
//     let stdout = fdt.chosen().stdout().unwrap();
//     assert!(stdout.node().name == "uart@10000000");
//     assert!(stdout.params() == Some("115200"));
// }

// #[test]
// fn stdin() {
//     let fdt = Fdt::new(TEST.as_slice()).unwrap();
//     let stdin = fdt.chosen().stdin().unwrap();
//     assert!(stdin.node().name == "uart@10000000");
//     assert!(stdin.params().is_none());
// }

// #[test]
// fn node_property_str_value() {
//     let fdt = Fdt::new(TEST.as_slice()).unwrap();
//     let cpu0 = fdt.find_node("/cpus/cpu@0").unwrap();
//     assert_eq!(cpu0.property("riscv,isa").unwrap().as_str().unwrap(), "rv64imafdcsu");
// }

// #[test]
// fn model_value() {
//     let fdt = Fdt::new(TEST.as_slice()).unwrap();
//     assert_eq!(fdt.root().model(), "riscv-virtio,qemu");
// }

// #[test]
// fn memory_node() {
//     let fdt = Fdt::new(TEST.as_slice()).unwrap();
//     assert_eq!(fdt.memory().regions().count(), 1);
// }

// #[test]
// fn interrupt_cells() {
//     let fdt = Fdt::new(TEST.as_slice()).unwrap();
//     let uart = fdt.find_node("/soc/uart").unwrap();
//     std::println!("{:?}", uart.parent_interrupt_cells());
//     assert_eq!(uart.interrupts().unwrap().collect::<std::vec::Vec<_>>(), std::vec![0xA]);
// }
