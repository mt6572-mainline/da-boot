# da-boot
Boot bare-metal binaries as DA on MediaTek MT6572

# Installation
- Make sure Rust and C++ compilers, python, cmake, make and `llvm-objcopy` are available on the host
- Install nightly toolchain: `rustup default nightly`
- Install host std: `rustup component add rust-src`
- Install arm std: `rustup target install armv7a-none-eabi`

# Subprojects
da-boot is modular enough to commonize code when need to.

Fill an issue if you want to re-use da-boot parts in your project. Note that some parts (e.g. protocol, bare-metal payloads and so on) are tied to the da-boot, and may introduce breaking changes. However, generic enough™ parts (like interceptor or bump allocator) can be moved out from the da-boot.

## Bare-metal side
- [Very tiny bump allocator](./payloads/bump)
- [DA1 exploit payload](./payloads/da1)
- [Thumb2 runtime hooking](./payloads/interceptor)
- [Helper payload (RPC)](./payloads/rpc)
- [Shared bits for bare-metal side](./payloads/shared)

## Host
- [proc-macro to generate communication with BootROM/Preloader/DA](./crates/da-boot-macros)
- [DA, LK and (partial) preloader parser](./crates/da-parser)
- [Preloader/DA static patcher](./crates/da-patcher)

## Mixed (both host and bare-metal)
- [da-boot's Protocol](./crates/da-protocol)
- [Multi-SoC support (WIP)](./crates/da-soc)

# Usage
## Booting generic bare-metal payload
### Prepare helper payload
```
cd payloads/rpc
./build.sh
cd ../..
```

## LK
Add `-m lk` to boot payload as LK image

### LK payload
LK can boot bare-metal payloads packed as boot.img (including U-Boot) via `boot_linux` function. The helper payload will hook it when more than one binary is uploaded in the LK mode (address must match temporary buffer for boot.img, so `0x83000000`).

## DA
The DA mode is currently work-in-progress and does nothing besides booting DA2 with an option test DA1 exploitation (DA2 is TODO).

Run `cargo r --release -p da-boot -- da -i /path/to/DA.bin`. To disable DA patching, pass the `--skip-patch` parameter.

## Parsing DA
Dump da1 and da2 for all SoCs
```
mkdir da
cargo r --release -p da-parser -- --input /path/to/DA.bin --output da
```
Add `--hw-code` parameter to filter the SoC

## Patching binaries
Note that patterns are tested only on few devices. Create an issue if something is failed to patch.

### Preloader
Strip 0xb00 bytes (preloader header) and run `cargo r --release -p da-patcher -- -i /path/to/preloader_without_header.bin -o preloader.bin --ty preloader`.

Most patches for preloader are removed because helper payload implements da-boot's own protocol instead of fixing MediaTek's.

### DA
Run `cargo r --release -p da-patcher -- -i /path/to/da1_or_da2.bin -o da1_or_da2.bin --ty da`.

This will enable UART output and disable hash check.

### Full command examples
Crash to the BootROM mode, boot `preloader.bin`, then `lk.bin` in FASTBOOT mode:
```
cargo r --release -p da-boot -- -c -p preloader.bin boot -i lk.bin -u 0x80020000 -j 0x80020000 -m lk --lk-mode fastboot
```
