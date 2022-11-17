#!/bin/bash

set -e

sudo ip netns exec spine1 gobgpd -f sartd/test/simple_rib/gobgp_spine1.conf &
sudo ip netns exec spine2 gobgpd -f sartd/test/simple_rib/gobgp_spine2.conf &

sleep 5
sudo ip netns exec spine1 gobgp global rib add -a ipv4 10.0.0.0/24 origin igp
sudo ip netns exec spine2 gobgp global rib add -a ipv4 10.0.1.0/24 origin igp 
