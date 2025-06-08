#!/bin/bash
clear
cargo clean
RUST_LOG=debug cargo run --bin backend