#![no_std]
#![no_main]
#![feature(used_with_arg)]

extern crate alloc;
extern crate bare_test;

// spinlock and interrupt
#[bare_test::tests]
mod tests {
    use bare_test::{
        GetIrqConfig,
        globals::{PlatformInfoKind, global_val},
        io::print,
        irq::{IrqHandleResult, IrqParam},
        mem::iomap,
        println,
    };
    use core::ops::{Deref, DerefMut};
    use core::{cell::UnsafeCell, str};
    use log::info;
    use pl011::Uart;
    pub const BAUD_RATE: u32 = 115200; // 波特率
    pub const CLK_RATE: u32 = 100_000_000; // 时钟频率

    // 实现一个spinlock
    pub struct Mutex<T> {
        inner: core::sync::atomic::AtomicBool,
        data: UnsafeCell<T>,
    }

    unsafe impl<T> Sync for Mutex<T> {}

    unsafe impl<T> Send for Mutex<T> {}

    impl<T> Mutex<T> {
        pub const fn new(data: T) -> Self {
            Self {
                inner: core::sync::atomic::AtomicBool::new(false),
                data: UnsafeCell::new(data),
            }
        }

        pub fn lock(&self) -> MutexGuard<'_, T> {
            while self.inner.swap(true, core::sync::atomic::Ordering::Acquire) {
                // busy-wait
            }
            MutexGuard { mutex: self }
        }

        pub fn unlock(&self) {
            self.inner
                .store(false, core::sync::atomic::Ordering::Release);
        }

        /// unsafe: this function can be used to get a mutable reference to the data
        #[allow(clippy::mut_from_ref)]
        pub unsafe fn force_use(&self) -> &mut T {
            unsafe { &mut *self.data.get() }
        }
    }

    pub struct MutexGuard<'a, T> {
        mutex: &'a Mutex<T>,
    }

    impl<'a, T> Deref for MutexGuard<'a, T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            unsafe { &*self.mutex.data.get() }
        }
    }

    impl<'a, T> DerefMut for MutexGuard<'a, T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            unsafe { &mut *self.mutex.data.get() }
        }
    }

    impl<'a, T> Drop for MutexGuard<'a, T> {
        fn drop(&mut self) {
            self.mutex.unlock();
        }
    }

    static UART: Mutex<Option<Uart>> = Mutex::new(None);

    #[test]
    fn it_works() {
        info!("This is a test log message.");
        let a = 2;
        let b = 2;

        assert_eq!(a + b, 4);
        let PlatformInfoKind::DeviceTree(fdt) = &global_val().platform_info;
        let fdt = fdt.get();
        let node = fdt.find_compatible(&["arm,pl011"]).next().unwrap();
        let irq_info = node.irq_info().unwrap();
        let cfg = irq_info.cfgs[0].clone();
        println!("UART IRQ: {:?}", irq_info);
        let reg = node.reg().unwrap().next().unwrap();
        let base = reg.address;
        let mut mmio = iomap((base as usize).into(), reg.size.unwrap());
        let uart = unsafe { Uart::new(mmio.as_mut() as *mut u8) };
        uart.init(BAUD_RATE, CLK_RATE);
        // 加锁，并通过括号自动drop锁
        {
            let mut pl011 = UART.lock();
            *pl011 = Some(uart);
        }
        // 注册中断函数
        IrqParam {
            intc: irq_info.irq_parent,
            cfg,
        }
        .register_builder(|_irq| {
            unsafe {
                UART.force_use().as_mut().unwrap().handle_interrupt();
            };

            IrqHandleResult::Handled
        })
        .register();

        // spin_on 持续循环等待
        {
            spin_on::spin_on(async {
                let mut pl011 = UART.lock();
                let uart = pl011.as_mut().unwrap();
                uart.write(b"Hello, async World!").await;
            });
        }
        println!("");

        println!("irq count: {}", UART.lock().as_ref().unwrap().irq_conut);

        println!("test passed!");
    }
}
