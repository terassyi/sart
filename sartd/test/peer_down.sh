#!/bin/bash

set -e
set -x

echo "==== Test to establish BGP session ===="

sartd/test/simple/topology.sh
sartd/test/simple/gobgp_run.sh

echo "==== RUN SARTD BGP ===="
sudo ip netns exec core sartd/target/debug/sartd -f sartd/test/simple/config.yaml &
