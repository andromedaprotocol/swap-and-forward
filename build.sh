RUSTFLAGS='-C link-arg=-s' cargo wasm
wasm-opt -Os ./target/wasm32-unknown-unknown/release/swap_and_forward.wasm -o ./artifacts/swap_and_forward.wasm
