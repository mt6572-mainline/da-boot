# da-boot
Boot U-Boot (or any other bare-metal binary) as DA on MediaTek MT6572

# Installation
- Make sure Rust and C++ compilers, python, cmake, make and `llvm-objcopy` are available on the host
- Install nightly toolchain: `rustup default nightly`
- Install host std: `rustup component add rust-src`
- Install arm std: `rustup target install armv7a-none-eabi`

# Usage
## Booting generic bare-metal payload
### Prepare payload
```
cd payloads/rpc
./build.sh
cd ../..
```

### Without preloader patcher
Note that only a single binary can be uploaded, LK images won't work and the payload must have `DA_DRAM_ADDR` base address:
```
cargo r --release -p da-boot -- boot -i bin -u 0x81e00000
```

### With brom patcher
Triggered when the crash option is enabled.

### With preloader patcher
Preloader is patched automatically when the upload address is not `DA_DRAM_ADDR`, upload binaries is more than 1 or LK image is specified. To forcefully run, pass the `-f` option.

You can also specify more payloads to upload like:
```
-i bin1 bin2 -u addr1 addr2 -j jumpaddr
```

### Debugging preloader patcher
Run `cargo r --release -p da-boot -- dump-preloader` to get `preloader.bin` file with patches applied.

Alternatively, [run da-patcher](#patching-preloader).

## LK
Add `-m lk` to boot payload as LK image

### LK payload
LK can boot bare-metal payloads (including U-Boot) via `boot_linux` function. The preloader patcher will hook it when more than one binary is uploaded in the LK mode (address must match LK kernel address, `0x80108000` for MT6572).

## DA
The DA mode is currently work-in-progress and does nothing besides booting DA2 with an option to patch both DA1 and DA2.

Run `cargo r --release -p da-boot -- boot-da -i /path/to/DA_PL.bin`. To disable DA patching, pass the `--quirky-preloader` parameter.

## Parsing DA
Dump da1 and da2 for all SoCs
```
mkdir da
cargo r --release -p da-parser -- --input /path/to/DA_PL.bin --output da
```
Add `--hw-code` parameter to filter the SoC

## Patching binaries
### Preloader
Strip 0xb00 bytes (preloader header) and run `cargo r --release -p da-patcher -- -i /path/to/preloader_without_header.bin -o preloader.bin --ty preloader`.

### DA
Run `cargo r --release -p da-patcher -- -i /path/to/da1_or_da2.bin -o da1_or_da2.bin --ty da`.
