cargo +nightly b --release -Z build-std=core,alloc
llvm-objcopy -O binary ../../target/armv7a-none-eabi/release/rpc ../../target/armv7a-none-eabi/release/rpc
