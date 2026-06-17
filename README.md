# da-boot
Boot binaries without flashing on MediaTek SoCs

# Installation
- Make sure `llvm-objcopy` is available on the host
- Install nightly toolchain: `rustup default nightly`
- Install host std: `rustup component add rust-src`
- Install arm std: `rustup target install armv7a-none-eabi`

# SoC support
- MT6572 is the former SoC and so has best support, tested across multiple devices
- MT6595 tested only on Lenovo Vibe X2

# Subprojects
da-boot is modular enough to commonize code when need to.

Fill an issue if you want to re-use da-boot parts in your project. Note that some parts (e.g. protocol, bare-metal payloads etc) are tied to da-boot, and may introduce breaking changes. However, generic enough™ parts (like interceptor or bump allocator) can be moved outside da-boot.

## Bare-metal side
- [Very tiny bump allocator](./payloads/bump)
- [Thumb2 runtime hooking](./payloads/interceptor)
- [Helper payload (RPC)](./payloads/rpc)
- [Shared bits for bare-metal side](./payloads/shared)

## Host
- [proc-macro to generate communication with BootROM/Preloader](./crates/da-boot-macros)
- [Preloader/LK static patcher (only extractions for now)](./crates/da-patcher)

## Mixed (both host and bare-metal)
- [da-boot's Protocol](./crates/da-protocol)
- [Multi-SoC support (WIP)](./crates/da-soc)

## Old subprojects
- da-port -> [simpleport](https://github.com/rva3/simpleport) - abstraction for serial communication
- da-analyzer -> [kaiko](https://github.com/rva3/kaiko) - ARM analysis for more reliable extraction/patching
- da-parser (+ lots of shomy's work) -> [hacc](https://github.com/shomykohai/hacc) - image parser & writer
- da-soc -> [acon](https://github.com/rva3/acon) - SoC db

# Usage
Make sure to supply preloader for **your exact device**, not from the other one.

## Prepare helper payload
```
cd payloads/rpc
./build.sh
cd ../..
```

## Boot modes
Run `cargo r --release -p da-boot -- --help` to see the full list.

### Preloader
Boot image after the preloader finishes execution.

#### Examples
- Boot LK: `cargo r --release -p da-boot -- --lk lk.bin -p preloader.bin preloader`
- Boot any other bare-metal image (custom headers won't be parsed): `cargo r --release -p da-boot -- --input target.bin@0xUPLOAD_ADDR -p preloader.bin preloader`

### LK
Boot image after the LK finishes execution. This will attempt to hook LK function to disable loading boot.img from the eMMC. The result may be wrong since this mode is experimental.

LK image is required to enter the mode. Make sure to use the one for **your exact device**, not from the others.

#### Examples
- Create boot.img with MediaTek headers and invoke mkbootimg: `cargo r --release -p da-boot -- --lk lk.bin --kernel zImageAndDTB.bin -p preloader.bin --dram-size-per-rank 0x20000000 --dram-ranks 1 lk`
- Boot already prepared boot.img: `cargo r --release -p da-boot -- --lk lk.bin --input boot.img@0xUPLOAD_ADDR -p preloader.bin --dram-size-per-rank 0x20000000 --dram-ranks 1 lk`. 0xUPLOAD_ADDR must be non-overlapping address in the DRAM, such as `0x85000000` for mt6572.

Note that `--dram-size-per-rank 0x20000000 --dram-ranks 1` works only for 512 MB devices, on 1 GB change `--dram-ranks` to `2`. For mt6595 DRAM size per rank is `0x40000000`.

# FAQ
- Does it work on my device?
- If the SoC is supported then maybe.

- Is it stable?
- It works until it doesn't.

- How do I debug crashes?
- If it's BootROM or Preloader stage then UART is required. You still can hack logs over USB, but I won't add this as a stable option. For LK stage it's possible to hack something for redirecting logs to the framebuffer like this. It's unlikely to be added as a stable feature in a near future, so UART is still highly recommended.
```rust
    hook! {
        fn videoprintf() {
            // uart prints
            sanitycheck::replace(0x8003C354 | 1);
            sanitycheck::replace(0x8003C960 | 1);
        }
    }

    hook! {
        fn sanitycheck(format: *const u8, r1: *const u8, r2: *const u8, r3: *const u8) {
            let s = CStr::from_ptr(format).to_string_lossy();
            uart_println!(s);
            if s.contains("DISP") || s.contains("fb") || s.contains("DDP") || s.contains("pitch") {
                return;
            }

            // video_printf
            c_function!(fn(*const u8, *const u8, *const u8, *const u8), 0x8003C148_u32 | 1)(format, r1, r2, r3);
        }
    }
```

**! beware !**

Some devices don't work with CP2102 adapters, some even with FT2232H. There's not much you can do with this. Neither I can recommend some specific UART-to-USB dongle (some of my devices don't work properly with FT2232H, some with CP2102).

- Why rust?
- C is a great langgRŢ�D�b��Segmentation fault (core dumped)

# Credits
- [kaeru](https://github.com/R0rt1z2/kaeru) - early C payload, macros
- [frida-gum](https://github.com/frida/frida-gum) - interceptor idea
- [hacc](https://github.com/shomykohai/hacc) - image parser & writer
- [kaiko](https://github.com/rva3/kaiko) - honorable mention for my ARM ISA sufferings
- [yaxpeax-arm](https://github.com/iximeow/yaxpeax-arm) - very fast and nice diassembler
- MediaTek - how you shouldn't do things :)
