# da-boot
Boot U-Boot (or any other bare-metal binary) as DA on MediaTek MT6572

# Usage
## Booting generic bare-metal payload
### Prepare payloads
```
cd payloads/preloader
./build.sh
cd ../..
cd payloads/brom
./build.sh
cd ../..
```

### With brom patcher
Triggered automatically when SBC, SLA or DAA is enabled or crash option is enabled.

Pass the `--only-brom` option to force booting payload in the SRAM (L2 cache) instead of the patched preloader.

### With preloader patcher
Preloader is patched automatically when the upload address is not `DA_DRAM_ADDR` or upload binaries is more than 1.

You can also specify more payloads to upload like:
```
-i bin1 bin2 -u addr1 addr2 -j jumpaddr
```

### Without preloader patcher
Note that only a single binary can be uploaded, lk images won't work and the payload must have `DA_DRAM_ADDR` base address:
```
cargo r --release -- -i bin -u 0x81e00000
```

### Debugging preloader patcher
Run `cargo r --release -- dump-preloader` to get preloader.bin file with patches applied

## LK
Add `-m lk` to boot payload as LK image

## Parsing DA
Dump da1 and da2 for all SoCs
```
mkdir da
cargo r --release -p da-parser -- --input /path/to/DA_PL.bin --output da
```
Add `--hw-code` parameter to filter the SoC

Sometimes `send_da` fails due to preloader sending garbage data (observed on MT8135 as well). If that happens, simply reset the device.

## Patching preloader
Strip 0xb00 bytes (preloader header) and run `cargo r --release -p da-patcher -- -i /path/to/preloader_without_header.bin -o preloader.bin`
