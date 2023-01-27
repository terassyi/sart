#!/bin/bash

set -e

sudo ip netns exec node1 gobgpd -f sartd/test/only_gobgp/gobgp_node1.conf &
sudo ip netns exec node2 gobgpd -f sartd/test/only_gobgp/gobgp_node2.conf &
sudo ip netns exec node3 gobgpd -f sartd/test/only_gobgp/gobgp_node3.conf &

sleep 5
# sudo ip netns exec node1 gobgp global rib add -a ipv4 10.0.0.0/24 origin igp
# sudo ip netns exec node2 gobgp global rib add -a ipv4 10.0.1.0/24 origin igp 
