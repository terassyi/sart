#!/bin/bash
set -e
set -x

sudo ip netns del core || true
sudo ip netns del spine1 || true
sudo ip netns del spine2 || true

sudo ip netns add core
sudo ip netns add spine1
sudo ip netns add spine2

sudo ip link add c-s1 type veth peer name s1-c
sudo ip link add c-s2 type veth peer name s2-c

sudo ip link set c-s1 netns core
sudo ip link set c-s2 netns core
sudo ip link set s1-c netns spine1
sudo ip link set s2-c netns spine2

sudo ip netns exec core ip addr add 1.1.1.1 dev lo
sudo ip netns exec spine1 ip addr add 2.2.2.2 dev lo
sudo ip netns exec spine2 ip addr add 3.3.3.3 dev lo

sudo ip netns exec core ip addr add 10.0.0.1/24 dev c-s1
sudo ip netns exec core ip addr add 10.0.1.1/24 dev c-s2
sudo ip netns exec spine1 ip addr add 10.0.0.2/24 dev s1-c
sudo ip netns exec spine2 ip addr add 10.0.1.2/24 dev s2-c

sudo ip netns exec core ip link set up dev lo
sudo ip netns exec spine1 ip link set up dev lo
sudo ip netns exec spine2 ip link set up dev lo

sudo ip netns exec core ip link set up dev c-s1
sudo ip netns exec core ip link set up dev c-s2
sudo ip netns exec spine1 ip link set up dev s1-c
sudo ip netns exec spine2 ip link set up dev s2-c

sudo ip netns exec core ip route add 2.2.2.2/32 via 10.0.0.1 dev c-s1
sudo ip netns exec core ip route add 3.3.3.3/32 via 10.0.1.1 dev c-s2
sudo ip netns exec spine1 ip route add 1.1.1.1/32 via 10.0.0.2 dev s1-c
sudo ip netns exec spine2 ip route add 1.1.1.1/32 via 10.0.1.2 dev s2-c
