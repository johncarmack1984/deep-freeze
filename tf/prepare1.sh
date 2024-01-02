#!/bin/bash
set -e

sudo apt-get update
# sudo NEEDRESTART_MODE=a apt install libssl-dev openssl --yes
sudo NEEDRESTART_MODE=a apt install libssl-dev --yes
sudo NEEDRESTART_MODE=a apt install openssl -y 
sudo NEEDRESTART_MODE=a apt-get upgrade -y
sudo NEEDRESTART_MODE=a apt install pkg-config -y 
sudo NEEDRESTART_MODE=a apt install awscli -y 
sudo NEEDRESTART_MODE=a apt install build-essential -y 

sudo shutdown -r now
