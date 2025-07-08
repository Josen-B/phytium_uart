#![no_std]
#![no_main]
#![feature(used_with_arg)]

extern crate alloc;
extern crate bare_test;

#[bare_test::tests]
mod tests {
    use bare_test::{
        globals::{PlatformInfoKind, global_val},
        mem::iomap,
        println,
    };
    use log::info;
    use pl011::Uart;
    pub const BAUD_RATE: u32 = 115200; // 波特率
    pub const CLK_RATE: u32 = 100_000_000; // 时钟频率

    #[test]
    fn it_works() {
        info!("This is a test log message.");
        let a = 2;
        let b = 2;
        assert_eq!(a + b, 4);

        let PlatformInfoKind::DeviceTree(fdt) = &global_val().platform_info;
        let fdt = fdt.get();
        let node = fdt.find_compatible(&["arm,pl011"]).next().unwrap();
        let reg = node.reg().unwrap().next().unwrap();
        let base = reg.address;
        let mut mmio = iomap((base as usize).into(), reg.size.unwrap());
        let mut uart = Uart::new(unsafe { mmio.as_mut() });
        uart.init(BAUD_RATE, CLK_RATE);
        for &c in b"Hello, world!\n".iter() {
            uart.send(c);
        }

        println!("test passed!");
    }
}
