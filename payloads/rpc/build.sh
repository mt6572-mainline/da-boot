cargo +nightly b --profile nostd -Z build-std=core,alloc
llvm-objcopy -O binary ../../target/armv7a-none-eabi/nostd/rpc ../../target/armv7a-none-eabi/nostd/rpc
