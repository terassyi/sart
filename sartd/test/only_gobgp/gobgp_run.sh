#!/bin/bash

set -e

sudo ip netns exec node1 gobgpd -f sartd/test/only_gobgp/gobgp_node1.conf &
sudo ip netns exec node2 gobgpd -f sartd/test/only_gobgp/gobgp_node2.conf &

sleep 1
sudo ip netns exec node1 gobgp neighbor add 10.0.0.1 as 100
sudo ip netns exec node2 gobgp neighbor add 10.0.0.2 as 200
