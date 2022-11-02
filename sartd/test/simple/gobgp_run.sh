#!/bin/bash
set -x
set -e

sudo ip netns exec spine1 gobgpd -f sartd/test/simple/gobgp_spine1.conf &
sudo ip netns exec spine2 gobgpd -f sartd/test/simple/gobgp_spine2.conf &

sleep 1
sudo ip netns exec spine1 gobgp neighbor add 10.0.0.1 as 100
sudo ip netns exec spine2 gobgp neighbor add 10.0.1.1 as 100
