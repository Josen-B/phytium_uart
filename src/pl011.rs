use core::{
    pin::Pin,
    ptr::NonNull,
    task::{Context, Poll},
};
use futures::task::AtomicWaker;
use log::{info, warn};
use tock_registers::{
    interfaces::{Readable, Writeable},
    register_bitfields, register_structs,
    registers::{ReadOnly, ReadWrite, WriteOnly},
};

register_structs! {
    UartRegs {
        (0x000 => uartdr: ReadWrite<u32, DATA::Register>),
        (0x004 => uartecr: ReadOnly<u32>),
        (0x008 => _reserved0),
        (0x018 => uartfr: ReadOnly<u32, FLAG::Register>),
        (0x01c => _reserved1),
        (0x024 => uartibrd: ReadWrite<u32>),
        (0x028 => uartfbrd: ReadWrite<u32>),
        (0x02c => uartlcrh: ReadWrite<u32>),
        (0x030 => uartcr: ReadWrite<u32>),
        (0x034 => uartifls: ReadWrite<u32, FIFO::Register>),
        (0x038 => uartimsc: ReadWrite<u32, INTERRUPT::Register>),
        (0x03c => uartris: ReadOnly<u32>),
        (0x040 => uartmis: ReadOnly<u32>),
        (0x044 => uarticr: WriteOnly<u32, ICR::Register>),
        (0x048 => uartdmacr: ReadWrite<u32>),
        (0x04c => @END),
    }
}

register_bitfields![u32,
    DATA [
        RAW OFFSET(0) NUMBITS(8),
        FE OFFSET(9) NUMBITS(1),
        PE OFFSET(10) NUMBITS(1),
        BE OFFSET(11) NUMBITS(1),
        OE OFFSET(12) NUMBITS(1),
    ],
    FLAG [
        CTS OFFSET(0) NUMBITS(1),
        DSR OFFSET(1) NUMBITS(1),
        DCD OFFSET(2) NUMBITS(1),
        BUSY OFFSET(3) NUMBITS(1),
        RXFE OFFSET(4) NUMBITS(1),
        TXFF OFFSET(5) NUMBITS(1),
        RXFF OFFSET(6) NUMBITS(1),
        TXFE OFFSET(7) NUMBITS(1),
    ],
    FIFO [
        TXSEL OFFSET(0) NUMBITS(3) [
            TX1_8 = 0,
            TX1_4 = 1,
            TX1_2 = 2,
            TX3_4 = 3,
            TX7_8 = 4,
        ],
        RXSEL OFFSET(3) NUMBITS(3) [
            RX1_8 = 0,
            RX1_4 = 1,
            RX1_2 = 2,
            RX3_4 = 3,
            RX7_8 = 4,
        ],
    ],
    INTERRUPT [
        RXIM OFFSET(4) NUMBITS(1),
        TXIM OFFSET(5) NUMBITS(1),
    ],
    ICR [
        RXIC OFFSET(4) NUMBITS(1),
        TXIC OFFSET(5) NUMBITS(1),
        RTIC OFFSET(6) NUMBITS(1),
        FEIC OFFSET(7) NUMBITS(1),
        PEIC OFFSET(8) NUMBITS(1),
        BEIC OFFSET(9) NUMBITS(1),
        OEIC OFFSET(10) NUMBITS(1),
    ]
];

pub struct Uart {
    pub base: NonNull<UartRegs>,
    waker: AtomicWaker,
    pub irq_conut: usize,
}

unsafe impl Send for Uart {}
unsafe impl Sync for Uart {}

impl Uart {
    pub const fn new(base: *mut u8) -> Self {
        Self {
            base: NonNull::new(base).unwrap().cast(),
            waker: AtomicWaker::new(),
            irq_conut: 0,
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
        // 启用中断
        uart.uartimsc.set(1 << 4 | 1 << 5);
        // 配置 UART
        info!("configuring UART");
        uart.uartlcrh.set(0x70); // 8位数据, 无奇偶校验, 1位停止位, FIFOs使能
        uart.uartcr.set(0x301); // 使能UART, 使能接收和发送
    }

    // 发送数据
    pub fn write<'a>(&'a mut self, data: &'a [u8]) -> impl Future<Output = usize> + 'a {
        WriteFuture {
            uart: self,
            data,
            index: 0,
        }
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

    pub fn handle_interrupt(&mut self) {
        self.irq_conut += 1;
        unsafe {
            if self.base.as_ref().uartfr.is_set(FLAG::RXFE) {
                self.waker.wake();
            }
            self.base.as_ref().uarticr.set(u32::MAX);
        }
    }
}

pub struct WriteFuture<'a> {
    uart: &'a Uart,
    data: &'a [u8],
    index: usize,
}

impl Future for WriteFuture<'_> {
    type Output = usize;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        unsafe {
            loop {
                if this.index >= this.data.len() {
                    return Poll::Ready(this.index);
                }

                if this.uart.base.as_ref().uartfr.get() & (1 << 5) != 0 {
                    this.uart.waker.register(_cx.waker());
                    return Poll::Pending;
                }

                let data = this.data[this.index];
                this.uart.base.as_ref().uartdr.set(data as u32);
                this.index += 1;
            }
        }
    }
}
