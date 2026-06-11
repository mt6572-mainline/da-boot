cargo +nightly b --bin brom --profile nostd -Z build-std=core,alloc -Zjson-target-spec
cargo +nightly b --bin pl --features pl --profile nostd -Z build-std=core,alloc -Zjson-target-spec
llvm-objcopy -O binary ../../target/armv7a-none-eabi/nostd/brom ../../target/armv7a-none-eabi/nostd/brom
llvm-objcopy -O binary ../../target/armv7a-none-eabi/nostd/pl ../../target/armv7a-none-eabi/nostd/pl
