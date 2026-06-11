# da-boot
Boot bare-metal binaries as DA on MediaTek MT6572

# Installation
- Make sure `llvm-objcopy` is available on the host
- Install nightly toolchain: `rustup default nightly`
- Install host std: `rustup component add rust-src`
- Install arm std: `rustup target install armv7a-none-eabi`

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
- [Preloader/DA static patcher (only extractions for now)](./crates/da-patcher)

## Mixed (both host and bare-metal)
- [da-boot's Protocol](./crates/da-protocol)
- [Multi-SoC support (WIP)](./crates/da-soc)

## Old subprojects
- da-port -> [simpleport](https://github.com/rva3/simpleport) - abstraction for serial communication
- da-analyzer -> [kaiko](https://github.com/rva3/kaiko) - ARM analysis for more reliable extraction/patching
- da-parser (+ lots of shomy's work) -> [hacc](https://github.com/shomykohai/hacc) - image parser & writer

### Pending moves
- da-soc -> [acon](https://github.com/rva3/acon/)

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
- Create boot.img with MediaTek headers and invoke mkbootimg: `cargo r --release -p da-boot -- --lk lk.bin --kernel zImageAndDTB.bin -p preloader.bin lk`
- Boot already prepared boot.img: `cargo r --release -p da-boot -- --lk lk.bin --input boot.img@0xUPLOAD_ADDR -p preloader.bin lk`. 0xUPLOAD_ADDR must be non-overlapping address in the DRAM, such as `0x85000000` for mt6572. 

## Credits
- [kaeru](https://github.com/R0rt1z2/kaeru) - early C payload, macros
- [frida-gum](https://github.com/frida/frida-gum) - interceptor idea
- [hacc](https://github.com/shomykohai/hacc) - image parser & writer
- [kaiko](https://github.com/rva3/kaiko) - honorable mention for my ARM ISA sufferings
- [yaxpeax-arm](https://github.com/iximeow/yaxpeax-arm) - very fast and nice diassembler
- MediaTek - how you shouldn't do things :)
