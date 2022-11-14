#!/bin/bash
set -e

sudo ip netns del node1 || true
sudo ip netns del node2 || true

sudo ip netns add node1
sudo ip netns add node2

sudo ip link add n1-n2 type veth peer name n2-n1

sudo ip link set n1-n2 netns node1
sudo ip link set n2-n1 netns node2

sudo ip netns exec node1 ip addr add 1.1.1.1 dev lo
sudo ip netns exec node2 ip addr add 2.2.2.2 dev lo

sudo ip netns exec node1 ip addr add 10.0.0.1/24 dev n1-n2
sudo ip netns exec node2 ip addr add 10.0.0.2/24 dev n2-n1

sudo ip netns exec node1 ip link set up dev lo
sudo ip netns exec node2 ip link set up dev lo

sudo ip netns exec node1 ip link set up dev n1-n2
sudo ip netns exec node2 ip link set up dev n2-n1

sudo ip netns exec node1 ip route add 2.2.2.2/32 via 10.0.0.1 dev n1-n2
sudo ip netns exec node2 ip route add 1.1.1.1/32 via 10.0.0.2 dev n2-n1
