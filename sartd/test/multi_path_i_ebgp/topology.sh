#!/bin/bash
set -e
set -x

sudo ip netns del node1 || true
sudo ip netns del node2 || true
sudo ip netns del node3 || true
sudo ip netns del node4 || true
sudo ip netns del node5 || true

sudo ip netns add node1
sudo ip netns add node2
sudo ip netns add node3
sudo ip netns add node4
sudo ip netns add node5

sudo ip link add n1-n2 type veth peer name n2-n1
sudo ip link add n1-n3 type veth peer name n3-n1
sudo ip link add n1-n4 type veth peer name n4-n1

sudo ip link add n2-n5 type veth peer name n5-n2
sudo ip link add n3-n5 type veth peer name n5-n3

sudo ip link add n2-d0 type dummy
sudo ip link add n4-d0 type dummy
sudo ip link add n5-d0 type dummy

sudo ip link set n1-n2 netns node1
sudo ip link set n2-n1 netns node2
sudo ip link set n1-n3 netns node1
sudo ip link set n3-n1 netns node3
sudo ip link set n1-n4 netns node1
sudo ip link set n4-n1 netns node4

sudo ip link set n5-n2 netns node5
sudo ip link set n2-n5 netns node2
sudo ip link set n5-n3 netns node5
sudo ip link set n3-n5 netns node3

sudo ip link set n2-d0 netns node2
sudo ip link set n4-d0 netns node4
sudo ip link set n5-d0 netns node5


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

sudo ip netns exec node2 ip addr add 10.10.0.2/24 dev n2-n5
sudo ip netns exec node2 ip addr add 10.100.0.2/24 dev n2-d0
sudo ip netns exec node3 ip addr add 10.12.0.2/24 dev n3-n5
sudo ip netns exec node4 ip addr add 10.11.0.2/24 dev n4-d0
sudo ip netns exec node5 ip addr add 10.10.0.4/24 dev n5-n2
sudo ip netns exec node5 ip addr add 10.12.0.5/24 dev n5-n3
sudo ip netns exec node5 ip addr add 10.69.0.100/32 dev n5-d0

sudo ip netns exec node1 ip link set up dev lo
sudo ip netns exec node2 ip link set up dev lo
sudo ip netns exec node3 ip link set up dev lo
sudo ip netns exec node4 ip link set up dev lo
sudo ip netns exec node5 ip link set up dev lo

sudo ip netns exec node1 ip link set up dev n1-n2
sudo ip netns exec node2 ip link set up dev n2-n1
sudo ip netns exec node1 ip link set up dev n1-n3
sudo ip netns exec node3 ip link set up dev n3-n1
sudo ip netns exec node1 ip link set up dev n1-n4
sudo ip netns exec node4 ip link set up dev n4-n1
sudo ip netns exec node2 ip link set up dev n2-n5
sudo ip netns exec node3 ip link set up dev n3-n5
sudo ip netns exec node5 ip link set up dev n5-n2
sudo ip netns exec node5 ip link set up dev n5-n3

sudo ip netns exec node2 ip link set up dev n2-d0
sudo ip netns exec node4 ip link set up dev n4-d0
sudo ip netns exec node5 ip link set up dev n5-d0

sudo ip netns exec node1 ip route add 2.2.2.2/32 via 10.0.0.1 dev n1-n2
sudo ip netns exec node2 ip route add 1.1.1.1/32 via 10.0.0.2 dev n2-n1
sudo ip netns exec node3 ip route add 3.3.3.3/32 via 10.0.1.2 dev n3-n1
sudo ip netns exec node4 ip route add 4.4.4.4/32 via 10.0.2.2 dev n4-n1

# sudo ip netns exec node2 ip route add 10.100.0.0/24 via 10.100.0.2 dev n2-d0
# sudo ip netns exec node4 ip route add 10.11.0.0/24 via 10.11.0.2 dev n4-d0
# sudo ip netns exec node5 ip route add 10.100.0.0/24 via  dev n5-d0

sudo ip netns exec node2 ip route add 10.69.0.0/24 via 10.10.0.2 dev n2-n5
sudo ip netns exec node3 ip route add 10.69.0.0/24 via 10.12.0.2 dev n3-n5
