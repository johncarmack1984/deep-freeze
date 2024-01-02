#!/bin/bash
set -e

sudo chown -R $(whoami) /home/ubuntu/*
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

git clone https://github.com/johncarmack1984/deep-freeze.git /home/ubuntu/deep-freeze

mv /home/ubuntu/.env /home/ubuntu/deep-freeze/.env
cd /home/ubuntu/deep-freeze
source $HOME/.cargo/env
cargo build --release

# sudo mv target/release/ /usr/local/bin && sudo chmod +x /usr/local/bin/deep-freeze
