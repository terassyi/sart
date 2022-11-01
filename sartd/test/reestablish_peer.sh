#!/bin/bash

set -e
set -x

echo "==== Test to establish BGP session ===="

sartd/test/simple/topology.sh
sartd/test/simple/gobgp_run.sh

echo "==== RUN SARTD BGP ===="
sudo ip netns exec core sartd/target/debug/sartd -f sartd/test/simple/config.yaml &

echo "==== SLEEP 10s ===="
sleep 10s

echo "==== STOP GOBGP ===="
sartd/test/simple/gobgp_stop.sh

echo "==== WAIT FOR RESTARTING SARTD PEER HANDLING"
sleep 30

echo "==== RESTART GOBGP ===="
sartd/test/simple/gobgp_run.sh

echo "==== SLEEP 10s ===="
sleep 10s

SPINE1_SESSION_STATE=$(sudo ip netns exec spine1 gobgp neighbor --json | jq .[].state.session_state)
RES=0

if [ $SPINE1_SESSION_STATE -eq 6 ]; then
	echo "  Core(sartd) <-> Spine1(gobgpd) is established"
	RES=$(expr $RES + 1)
else
	echo "  FAILED!!! Core(sartd) <-> Spine1(gobgpd) is not established"
fi

SPINE2_SESSION_STATE=$(sudo ip netns exec spine1 gobgp neighbor --json | jq .[].state.session_state)

if [ $SPINE2_SESSION_STATE -eq 6 ]; then
	echo "  Core(sartd) <-> Spine2(gobgpd) is established"
	RES=$(expr $RES + 1)
else
	echo "  FAILED!!! Core(sartd) <-> Spine2(gobgpd) is not established"
fi

echo "==== CREANUP ===="
sartd/test/simple/gobgp_stop.sh
sartd/test/sartd_stop.sh
sartd/test/simple/clean_topology.sh

if [ $RES -ne 2 ]; then
	false
fi
