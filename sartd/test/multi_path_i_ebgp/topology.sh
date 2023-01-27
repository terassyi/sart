#!/bin/bash
set -e

sudo ip netns del node1 || true
sudo ip netns del node2 || true
sudo ip netns del node3 || true
sudo ip netns del node4 || true

sudo ip netns add node1
sudo ip netns add node2
sudo ip netns add node3
sudo ip netns add node4

sudo ip link add n1-n2 type veth peer name n2-n1
sudo ip link add n1-n3 type veth peer name n3-n1
sudo ip link add n1-n4 type veth peer name n4-n1

sudo ip link set n1-n2 netns node1
sudo ip link set n2-n1 netns node2
sudo ip link set n1-n3 netns node1
sudo ip link set n3-n1 netns node3
sudo ip link set n1-n4 netns node1
sudo ip link set n4-n1 netns node4


sudo ip netns exec node1 ip addr add 1.1.1.1 dev lo
sudo ip netns exec node2 ip addr add 2.2.2.2 dev lo
sudo ip netns exec node3 ip addr add 3.3.3.3 dev lo
sudo ip netns exec node4 ip addr add 4.4.4.4 dev lo

sudo ip netns exec node1 ip addr add 10.0.0.1/24 dev n1-n2
sudo ip netns exec node2 ip addr add 10.0.0.2/24 dev n2-n1
sudo ip netns exec node1 ip addr add 10.0.1.1/24 dev n1-n3
sudo ip netns exec node3 ip addr add 10.0.1.2/24 dev n3-n1
sudo ip netns exec node1 ip addr add 10.0.2.1/24 dev n1-n4
sudo ip netns exec node4 ip addr add 10.0.2.2/24 dev n4-n1

sudo ip netns exec node1 ip link set up dev lo
sudo ip netns exec node2 ip link set up dev lo
sudo ip netns exec node3 ip link set up dev lo
sudo ip netns exec node4 ip link set up dev lo

sudo ip netns exec node1 ip link set up dev n1-n2
sudo ip netns exec node2 ip link set up dev n2-n1
sudo ip netns exec node1 ip link set up dev n1-n3
sudo ip netns exec node3 ip link set up dev n3-n1
sudo ip netns exec node1 ip link set up dev n1-n4
sudo ip netns exec node4 ip link set up dev n4-n1

sudo ip netns exec node1 ip route add 2.2.2.2/32 via 10.0.0.1 dev n1-n2
sudo ip netns exec node2 ip route add 1.1.1.1/32 via 10.0.0.2 dev n2-n1
sudo ip netns exec node3 ip route add 3.3.3.3/32 via 10.0.1.2 dev n3-n1
sudo ip netns exec node4 ip route add 4.4.4.4/32 via 10.0.2.2 dev n4-n1
