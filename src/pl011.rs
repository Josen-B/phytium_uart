use core::ptr::NonNull;
use log::{info, warn};
use tock_registers::{
    interfaces::{Readable, Writeable},
    register_structs,
    registers::{ReadOnly, ReadWrite, WriteOnly},
};

register_structs! {
    UartRegs {
        (0x000 => uartdr: ReadWrite<u32>),
        (0x004 => uartecr: ReadOnly<u32>),
        (0x008 => _reserved0),
        (0x018 => uartfr: ReadOnly<u32>),
        (0x01c => _reserved1),
        (0x024 => uartibrd: ReadWrite<u32>),
        (0x028 => uartfbrd: ReadWrite<u32>),
        (0x02c => uartlcrh: ReadWrite<u32>),
        (0x030 => uartcr: ReadWrite<u32>),
        (0x034 => uartifls: ReadWrite<u32>),
        (0x038 => uartimsc: ReadWrite<u32>),
        (0x03c => uartris: ReadOnly<u32>),
        (0x040 => uartmis: ReadOnly<u32>),
        (0x044 => uarticr: WriteOnly<u32>),
        (0x048 => uartdmacr: ReadWrite<u32>),
        (0x04c => @END),
    }
}

pub struct Uart {
    pub base: NonNull<UartRegs>,
}

unsafe impl Send for Uart {}
unsafe impl Sync for Uart {}

impl Uart {
    pub const fn new(base: *mut u8) -> Self {
        Self {
            base: NonNull::new(base).unwrap().cast(),
        }
    }

    pub fn init(&self, clk_rate: u32, baud_rate: u32) {
        let uart = unsafe { self.base.as_ref() };
        // 关闭 UART
        uart.uartcr.set(0);
        // 设置波特率
        let integer_part = clk_rate / (16 * baud_rate);
        let fraction_part = ((clk_rate % (16 * baud_rate)) * 64 / (16 * baud_rate)) as u8;
        info!(
            "integer_part is {}, fraction_part is {}",
            integer_part, fraction_part
        );
        uart.uartibrd.set(integer_part);
        uart.uartfbrd.set(fraction_part as u32);
        // 使能fifo
        uart.uartifls.set(0x20);
        // 配置 UART
        info!("configuring UART");
        uart.uartlcrh.set(0x70); // 8位数据, 无奇偶校验, 1位停止位, FIFOs使能
        uart.uartcr.set(0x301); // 使能UART, 使能接收和发送
    }

    // 发送数据
    pub fn send(&self, data: u8) {
        let uart = unsafe { self.base.as_ref() };
        if uart.uartfr.get() & (1 << 5) == 0 {
            //info!("FIFO not empty, waiting to send data");
        }
        uart.uartdr.set(data as u32);
    }

    // 接收数据
    pub fn receive(&self) -> u8 {
        let uart = unsafe { self.base.as_ref() };
        if uart.uartfr.get() & (1 << 5) != 0 {
            warn!("FIFO is empty, no data to receive");
            return 0; // 或者返回一个错误值
        }
        uart.uartdr.get() as u8
    }
}
