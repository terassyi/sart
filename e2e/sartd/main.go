package main

import (
	"context"
	"fmt"
	"sort"

	"github.com/spf13/cobra"
	"github.com/terassyi/sart/e2e/topology"
)

var TopologyMap = map[string]topology.Topology{
	"simple":                                     simpleTopology,
	"simple-without-config":                      simpleTopologyWithoutConfig,
	"simple-with-zebra":                          simpleTopologyWithZebra,
	"simple-ibgp-with-zebra":                     simpleTopologyIBGPWithZebra,
	"simple-multipath":                           multiPathSimple,
	"simple-multipath-with-zebra":                multiPathSimpleWithZebra,
	"simple-with-fib":                            simpleTopologyWithClient,
	"simple-with-fib-another-table":              simpleTopologyWithClientAnotherTable,
	"simple-with-fib-subscribe-kernel":           simpleTopologyWithClientSubscribeKernel,
	"simple-with-fib-subscribe-kernel-no-daemon": simpleTopologyWithClientSubscribeKernelNoDaemon,
	"simple-no-daemon":                           simpleTopologyNoDaemon,
}

var simpleTopology topology.Topology = topology.Topology{
	Name: "simple",
	Nodes: []topology.Node{
		{
			Name: "node2",
			Interfaces: []topology.Interface{
				{
					Name:     "n2-n1",
					PeerName: "1-2",
					Address:  "10.0.0.3",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node2",
					Destination: "/etc/node2",
				},
			},
			Privileged:   true,
			InitCommands: []string{"gobgpd", "-f", "/etc/node2/config.toml"},
		},
		{
			Name: "node1",
			Interfaces: []topology.Interface{
				{
					Name:     "n1-n2",
					PeerName: "1-2",
					Address:  "10.0.0.2",
				},
				{
					Name:     "n1-n3",
					PeerName: "1-3",
					Address:  "10.0.1.2",
				},
			},
			Image: "sart:dev",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node1",
					Destination: "/etc/node1",
				},
			},
			Privileged:   true,
			InitCommands: []string{"sartd", "bgp", "-f", "/etc/node1/config.yaml"},
			Config:       "",
			Commands:     []string{},
		},
		{
			Name: "node3",
			Interfaces: []topology.Interface{
				{
					Name:     "n3-n1",
					PeerName: "1-3",
					Address:  "10.0.1.3",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node3",
					Destination: "/etc/node3",
				},
			},
			Privileged:   true,
			InitCommands: []string{"gobgpd", "-f", "/etc/node3/config.toml"},
		},
	},
	Peers: []topology.Peer{
		{
			Name: "1-2",
			Cidr: "10.0.0.0/24",
		},
		{
			Name: "1-3",
			Cidr: "10.0.1.0/24",
		},
	},
}

var simpleTopologyNoDaemon topology.Topology = topology.Topology{
	Name: "simple",
	Nodes: []topology.Node{
		{
			Name: "node2",
			Interfaces: []topology.Interface{
				{
					Name:     "n2-n1",
					PeerName: "1-2",
					Address:  "10.0.0.3",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/multipath_simple/node2",
					Destination: "/etc/node2",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"/usr/lib/frr/frrinit.sh start",
				"sleep 30",
				"gobgpd -f /etc/node2/config_zebra.toml",
			},
		},
		{
			Name: "node1",
			Interfaces: []topology.Interface{
				{
					Name:     "n1-n2",
					PeerName: "1-2",
					Address:  "10.0.0.2",
				},
				{
					Name:     "n1-n3",
					PeerName: "1-3",
					Address:  "10.0.1.2",
				},
			},
			Image: "sart:dev",
			Volume: []topology.Volume{
				{
					Source:      "./data/multipath_simple/node1",
					Destination: "/etc/node1",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Config:       "",
			Commands:     []string{},
		},
		{
			Name: "node3",
			Interfaces: []topology.Interface{
				{
					Name:     "n3-n1",
					PeerName: "1-3",
					Address:  "10.0.1.3",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/multipath_simple/node3",
					Destination: "/etc/node3",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"/usr/lib/frr/frrinit.sh start",
				"sleep 30",
				"gobgpd -f /etc/node3/config_zebra.toml",
			},
		},
	},
	Peers: []topology.Peer{
		{
			Name: "1-2",
			Cidr: "10.0.0.0/24",
		},
		{
			Name: "1-3",
			Cidr: "10.0.1.0/24",
		},
	},
}

var simpleTopologyWithoutConfig topology.Topology = topology.Topology{
	Name: "simple",
	Nodes: []topology.Node{
		{
			Name: "node1",
			Interfaces: []topology.Interface{
				{
					Name:     "n1-n2",
					PeerName: "1-2",
					Address:  "10.0.0.2",
				},
				{
					Name:     "n1-n3",
					PeerName: "1-3",
					Address:  "10.0.1.2",
				},
			},
			Image: "sart:dev",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node1",
					Destination: "/etc/node1",
				},
			},
			Privileged:   true,
			InitCommands: []string{"sartd", "bgp"},
			Config:       "",
			Commands:     []string{},
		},
		{
			Name: "node2",
			Interfaces: []topology.Interface{
				{
					Name:     "n2-n1",
					PeerName: "1-2",
					Address:  "10.0.0.3",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node2",
					Destination: "/etc/node2",
				},
			},
			Privileged:   true,
			InitCommands: []string{"gobgpd", "-f", "/etc/node2/config.toml"},
		},
		{
			Name: "node3",
			Interfaces: []topology.Interface{
				{
					Name:     "n3-n1",
					PeerName: "1-3",
					Address:  "10.0.1.3",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node3",
					Destination: "/etc/node3",
				},
			},
			Privileged:   true,
			InitCommands: []string{"gobgpd", "-f", "/etc/node3/config.toml"},
		},
	},
	Peers: []topology.Peer{
		{
			Name: "1-2",
			Cidr: "10.0.0.0/24",
		},
		{
			Name: "1-3",
			Cidr: "10.0.1.0/24",
		},
	},
}

var simpleTopologyWithZebra topology.Topology = topology.Topology{
	Name: "simple",
	Nodes: []topology.Node{
		{
			Name: "node1",
			Interfaces: []topology.Interface{
				{
					Name:     "n1-n2",
					PeerName: "1-2",
					Address:  "10.0.0.2",
				},
				{
					Name:     "n1-n3",
					PeerName: "1-3",
					Address:  "10.0.1.2",
				},
			},
			Image: "sart:dev",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node1",
					Destination: "/etc/node1",
				},
			},
			Privileged:   true,
			InitCommands: []string{"sartd", "bgp", "-f", "/etc/node1/config.yaml"},
			Config:       "",
		},
		{
			Name: "node2",
			Interfaces: []topology.Interface{
				{
					Name:     "n2-n1",
					PeerName: "1-2",
					Address:  "10.0.0.3",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node2",
					Destination: "/etc/node2",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"/usr/lib/frr/frrinit.sh start",
				"sleep 30",
				"gobgpd -f /etc/node2/config_zebra.toml",
			},
		},
		{
			Name: "node3",
			Interfaces: []topology.Interface{
				{
					Name:     "n3-n1",
					PeerName: "1-3",
					Address:  "10.0.1.3",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node3",
					Destination: "/etc/node3",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"/usr/lib/frr/frrinit.sh start",
				"sleep 30",
				"gobgpd -f /etc/node3/config_zebra.toml",
			},
		},
	},
	Peers: []topology.Peer{
		{
			Name: "1-2",
			Cidr: "10.0.0.0/24",
		},
		{
			Name: "1-3",
			Cidr: "10.0.1.0/24",
		},
	},
}

var simpleTopologyIBGPWithZebra topology.Topology = topology.Topology{
	Name: "simple",
	Nodes: []topology.Node{
		{
			Name: "node1",
			Interfaces: []topology.Interface{
				{
					Name:     "n1-n2",
					PeerName: "1-2",
					Address:  "10.0.0.2",
				},
				{
					Name:     "n1-n3",
					PeerName: "1-3",
					Address:  "10.0.1.2",
				},
			},
			Image: "sart:dev",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node1",
					Destination: "/etc/node1",
				},
			},
			Privileged:   true,
			InitCommands: []string{"sartd", "bgp", "-f", "/etc/node1/config_ibgp.yaml"},
			Config:       "",
		},
		{
			Name: "node2",
			Interfaces: []topology.Interface{
				{
					Name:     "n2-n1",
					PeerName: "1-2",
					Address:  "10.0.0.3",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node2",
					Destination: "/etc/node2",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"/usr/lib/frr/frrinit.sh start",
				"sleep 30",
				"gobgpd -f /etc/node2/config_ibgp_zebra.toml",
			},
		},
		{
			Name: "node3",
			Interfaces: []topology.Interface{
				{
					Name:     "n3-n1",
					PeerName: "1-3",
					Address:  "10.0.1.3",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node3",
					Destination: "/etc/node3",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"/usr/lib/frr/frrinit.sh start",
				"sleep 30",
				"gobgpd -f /etc/node3/config_ibgp_zebra.toml",
			},
		},
	},
	Peers: []topology.Peer{
		{
			Name: "1-2",
			Cidr: "10.0.0.0/24",
		},
		{
			Name: "1-3",
			Cidr: "10.0.1.0/24",
		},
	},
}

var simpleTopologyWithClient topology.Topology = topology.Topology{
	Name: "simple",
	Nodes: []topology.Node{
		{
			Name: "node1",
			Interfaces: []topology.Interface{
				{
					Name:     "n1-n2",
					PeerName: "1-2",
					Address:  "10.0.0.2",
				},
				{
					Name:     "n1-n3",
					PeerName: "1-3",
					Address:  "10.0.1.2",
				},
				{
					Name:     "n1-n5",
					PeerName: "1-5",
					Address:  "10.0.11.2",
				},
			},
			Image: "sart:dev",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node1",
					Destination: "/etc/node1",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Config:       "",
			Commands: []string{
				"sartd fib -f /etc/node1/fib.yaml",
				"sartd bgp -f /etc/node1/config.yaml --fib localhost:5010",
			},
		},
		{
			Name: "node2",
			Interfaces: []topology.Interface{
				{
					Name:     "n2-n1",
					PeerName: "1-2",
					Address:  "10.0.0.3",
				},
				{
					Name:     "n2-n4",
					PeerName: "2-4",
					Address:  "10.0.10.2",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node2",
					Destination: "/etc/node2",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"/usr/lib/frr/frrinit.sh start",
				"sleep 30",
				"gobgpd -f /etc/node2/config_zebra.toml",
			},
		},
		{
			Name: "node3",
			Interfaces: []topology.Interface{
				{
					Name:     "n3-n1",
					PeerName: "1-3",
					Address:  "10.0.1.3",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node3",
					Destination: "/etc/node3",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"/usr/lib/frr/frrinit.sh start",
				"sleep 30",
				"gobgpd -f /etc/node3/config_zebra.toml",
			},
		},
		{
			Name: "node4",
			Interfaces: []topology.Interface{
				{
					Name:     "n4-n2",
					PeerName: "2-4",
					Address:  "10.0.10.3",
				},
			},
			Image:        "ghcr.io/terassyi/terakoya:0.1.2",
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"ip route change default via 10.0.10.2",
			},
		},
		{
			Name: "node5",
			Interfaces: []topology.Interface{
				{
					Name:     "n5-n1",
					PeerName: "1-5",
					Address:  "10.0.11.3",
				},
			},
			Image:        "ghcr.io/terassyi/terakoya:0.1.2",
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"ip route change default via 10.0.11.2",
			},
		},
	},
	Peers: []topology.Peer{
		{
			Name: "1-2",
			Cidr: "10.0.0.0/24",
		},
		{
			Name: "1-3",
			Cidr: "10.0.1.0/24",
		},
		{
			Name: "2-4",
			Cidr: "10.0.10.0/24",
		},
		{
			Name: "1-5",
			Cidr: "10.0.11.0/24",
		},
	},
}

var simpleTopologyWithClientAnotherTable topology.Topology = topology.Topology{
	Name: "simple",
	Nodes: []topology.Node{
		{
			Name: "node1",
			Interfaces: []topology.Interface{
				{
					Name:     "n1-n2",
					PeerName: "1-2",
					Address:  "10.0.0.2",
				},
				{
					Name:     "n1-n3",
					PeerName: "1-3",
					Address:  "10.0.1.2",
				},
				{
					Name:     "n1-n5",
					PeerName: "1-5",
					Address:  "10.0.11.2",
				},
			},
			Image: "sart:dev",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node1",
					Destination: "/etc/node1",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Config:       "",
			Commands: []string{
				"ip rule add table 100 priority 100",
				"sartd fib -f /etc/node1/fib_another_table.yaml",
				"sartd bgp -f /etc/node1/config.yaml --fib localhost:5010",
			},
		},
		{
			Name: "node2",
			Interfaces: []topology.Interface{
				{
					Name:     "n2-n1",
					PeerName: "1-2",
					Address:  "10.0.0.3",
				},
				{
					Name:     "n2-n4",
					PeerName: "2-4",
					Address:  "10.0.10.2",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node2",
					Destination: "/etc/node2",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"/usr/lib/frr/frrinit.sh start",
				"sleep 30",
				"gobgpd -f /etc/node2/config_zebra.toml",
			},
		},
		{
			Name: "node3",
			Interfaces: []topology.Interface{
				{
					Name:     "n3-n1",
					PeerName: "1-3",
					Address:  "10.0.1.3",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node3",
					Destination: "/etc/node3",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"/usr/lib/frr/frrinit.sh start",
				"sleep 30",
				"gobgpd -f /etc/node3/config_zebra.toml",
			},
		},
		{
			Name: "node4",
			Interfaces: []topology.Interface{
				{
					Name:     "n4-n2",
					PeerName: "2-4",
					Address:  "10.0.10.3",
				},
			},
			Image:        "ghcr.io/terassyi/terakoya:0.1.2",
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"ip route change default via 10.0.10.2",
			},
		},
		{
			Name: "node5",
			Interfaces: []topology.Interface{
				{
					Name:     "n5-n1",
					PeerName: "1-5",
					Address:  "10.0.11.3",
				},
			},
			Image:        "ghcr.io/terassyi/terakoya:0.1.2",
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"ip route change default via 10.0.11.2",
			},
		},
	},
	Peers: []topology.Peer{
		{
			Name: "1-2",
			Cidr: "10.0.0.0/24",
		},
		{
			Name: "1-3",
			Cidr: "10.0.1.0/24",
		},
		{
			Name: "2-4",
			Cidr: "10.0.10.0/24",
		},
		{
			Name: "1-5",
			Cidr: "10.0.11.0/24",
		},
	},
}

var simpleTopologyWithClientSubscribeKernel topology.Topology = topology.Topology{
	Name: "simple",
	Nodes: []topology.Node{
		{
			Name: "node1",
			Interfaces: []topology.Interface{
				{
					Name:     "n1-n2",
					PeerName: "1-2",
					Address:  "10.0.0.2",
				},
				{
					Name:     "n1-n3",
					PeerName: "1-3",
					Address:  "10.0.1.2",
				},
				{
					Name:     "n1-n5",
					PeerName: "1-5",
					Address:  "10.0.11.2",
				},
			},
			Image: "sart:dev",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node1",
					Destination: "/etc/node1",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Config:       "",
			Commands: []string{
				"ip rule add table 100 priority 100",
				"sartd fib -f /etc/node1/fib_subscribe_kernel.yaml",
				"sartd bgp -f /etc/node1/config.yaml --fib localhost:5010",
			},
		},
		{
			Name: "node2",
			Interfaces: []topology.Interface{
				{
					Name:     "n2-n1",
					PeerName: "1-2",
					Address:  "10.0.0.3",
				},
				{
					Name:     "n2-n4",
					PeerName: "2-4",
					Address:  "10.0.10.2",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node2",
					Destination: "/etc/node2",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"/usr/lib/frr/frrinit.sh start",
				"sleep 30",
				"gobgpd -f /etc/node2/config_zebra.toml",
			},
		},
		{
			Name: "node3",
			Interfaces: []topology.Interface{
				{
					Name:     "n3-n1",
					PeerName: "1-3",
					Address:  "10.0.1.3",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node3",
					Destination: "/etc/node3",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"/usr/lib/frr/frrinit.sh start",
				"sleep 30",
				"gobgpd -f /etc/node3/config_zebra.toml",
			},
		},
		{
			Name: "node4",
			Interfaces: []topology.Interface{
				{
					Name:     "n4-n2",
					PeerName: "2-4",
					Address:  "10.0.10.3",
				},
			},
			Image:        "ghcr.io/terassyi/terakoya:0.1.2",
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"ip route change default via 10.0.10.2",
			},
		},
		{
			Name: "node5",
			Interfaces: []topology.Interface{
				{
					Name:     "n5-n1",
					PeerName: "1-5",
					Address:  "10.0.11.3",
				},
			},
			Image:        "ghcr.io/terassyi/terakoya:0.1.2",
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"ip addr add 5.5.5.5/32 dev lo",
				"ip route change default via 10.0.11.2",
			},
		},
	},
	Peers: []topology.Peer{
		{
			Name: "1-2",
			Cidr: "10.0.0.0/24",
		},
		{
			Name: "1-3",
			Cidr: "10.0.1.0/24",
		},
		{
			Name: "2-4",
			Cidr: "10.0.10.0/24",
		},
		{
			Name: "1-5",
			Cidr: "10.0.11.0/24",
		},
	},
}

var simpleTopologyWithClientSubscribeKernelNoDaemon topology.Topology = topology.Topology{
	Name: "simple",
	Nodes: []topology.Node{
		{
			Name: "node1",
			Interfaces: []topology.Interface{
				{
					Name:     "n1-n2",
					PeerName: "1-2",
					Address:  "10.0.0.2",
				},
				{
					Name:     "n1-n3",
					PeerName: "1-3",
					Address:  "10.0.1.2",
				},
				{
					Name:     "n1-n5",
					PeerName: "1-5",
					Address:  "10.0.11.2",
				},
			},
			Image: "sart:dev",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node1",
					Destination: "/etc/node1",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Config:       "",
			Commands: []string{
				"ip rule add table 100 priority 100",
			},
		},
		{
			Name: "node2",
			Interfaces: []topology.Interface{
				{
					Name:     "n2-n1",
					PeerName: "1-2",
					Address:  "10.0.0.3",
				},
				{
					Name:     "n2-n4",
					PeerName: "2-4",
					Address:  "10.0.10.2",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node2",
					Destination: "/etc/node2",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"/usr/lib/frr/frrinit.sh start",
				"sleep 30",
				"gobgpd -f /etc/node2/config_zebra.toml",
			},
		},
		{
			Name: "node3",
			Interfaces: []topology.Interface{
				{
					Name:     "n3-n1",
					PeerName: "1-3",
					Address:  "10.0.1.3",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node3",
					Destination: "/etc/node3",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"/usr/lib/frr/frrinit.sh start",
				"sleep 30",
				"gobgpd -f /etc/node3/config_zebra.toml",
			},
		},
		{
			Name: "node4",
			Interfaces: []topology.Interface{
				{
					Name:     "n4-n2",
					PeerName: "2-4",
					Address:  "10.0.10.3",
				},
			},
			Image:        "ghcr.io/terassyi/terakoya:0.1.2",
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"ip route change default via 10.0.10.2",
			},
		},
		{
			Name: "node5",
			Interfaces: []topology.Interface{
				{
					Name:     "n5-n1",
					PeerName: "1-5",
					Address:  "10.0.11.3",
				},
			},
			Image:        "ghcr.io/terassyi/terakoya:0.1.2",
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"ip addr add 5.5.5.5/32 dev lo",
				"ip route change default via 10.0.11.2",
			},
		},
	},
	Peers: []topology.Peer{
		{
			Name: "1-2",
			Cidr: "10.0.0.0/24",
		},
		{
			Name: "1-3",
			Cidr: "10.0.1.0/24",
		},
		{
			Name: "2-4",
			Cidr: "10.0.10.0/24",
		},
		{
			Name: "1-5",
			Cidr: "10.0.11.0/24",
		},
	},
}

var multiPathSimple topology.Topology = topology.Topology{
	Name: "multi-path",
	Nodes: []topology.Node{
		{
			Name: "node1",
			Interfaces: []topology.Interface{
				{
					Name:     "n1-n2",
					PeerName: "1-2",
					Address:  "10.0.0.2",
				},
				{
					Name:     "n1-n3",
					PeerName: "1-3",
					Address:  "10.0.1.2",
				},
			},
			Image: "sart:dev",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node1",
					Destination: "/etc/node1",
				},
			},
			Privileged:   true,
			InitCommands: []string{"sartd", "bgp", "-f", "/etc/node1/config.yaml"},
			Config:       "",
			Commands:     []string{},
		},
		{
			Name: "node2",
			Interfaces: []topology.Interface{
				{
					Name:     "n2-n1",
					PeerName: "1-2",
					Address:  "10.0.0.3",
				},
				{
					Name:     "n2-n4",
					PeerName: "1-2",
					Address:  "10.0.2.2",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node2",
					Destination: "/etc/node2",
				},
			},
			Privileged:   true,
			InitCommands: []string{"gobgpd", "-f", "/etc/node2/config.toml"},
		},
		{
			Name: "node3",
			Interfaces: []topology.Interface{
				{
					Name:     "n3-n1",
					PeerName: "1-3",
					Address:  "10.0.1.3",
				},
				{
					Name:     "n3-n4",
					PeerName: "3-4",
					Address:  "10.0.3.2",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node3",
					Destination: "/etc/node3",
				},
			},
			Privileged:   true,
			InitCommands: []string{"gobgpd", "-f", "/etc/node3/config.toml"},
		},
		{
			Name: "node4",
			Interfaces: []topology.Interface{
				{
					Name:     "n4-n2",
					PeerName: "2-4",
					Address:  "10.0.2.3",
				},
				{
					Name:     "n4-n3",
					PeerName: "3-4",
					Address:  "10.0.3.3",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/simple/node3",
					Destination: "/etc/node3",
				},
			},
			Privileged:   true,
			InitCommands: []string{"gobgpd", "-f", "/etc/node3/config.toml"},
		},
	},
	Peers: []topology.Peer{
		{
			Name: "1-2",
			Cidr: "10.0.0.0/24",
		},
		{
			Name: "1-3",
			Cidr: "10.0.1.0/24",
		},
		{
			Name: "2-4",
			Cidr: "10.0.2.0/24",
		},
		{
			Name: "3-4",
			Cidr: "10.0.3.0/24",
		},
	},
}

var multiPathSimpleWithZebra topology.Topology = topology.Topology{
	Name: "multi-path-zebra",
	Nodes: []topology.Node{
		{
			Name: "node1",
			Interfaces: []topology.Interface{
				{
					Name:     "n1-n2",
					PeerName: "1-2",
					Address:  "10.0.0.2",
				},
				{
					Name:     "n1-n3",
					PeerName: "1-3",
					Address:  "10.0.1.2",
				},
				{
					Name:     "n1-n5",
					PeerName: "1-5",
					Address:  "10.0.11.2",
				},
			},
			Image: "sart:dev",
			Volume: []topology.Volume{
				{
					Source:      "./data/multipath_simple/node1",
					Destination: "/etc/node1",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Config:       "",
			Commands: []string{
				"sartd fib -f /etc/node1/fib.yaml",
				"sartd bgp -f /etc/node1/config.yaml --fib localhost:5010",
			},
		},
		{
			Name: "node2",
			Interfaces: []topology.Interface{
				{
					Name:     "n2-n1",
					PeerName: "1-2",
					Address:  "10.0.0.3",
				},
				{
					Name:     "n2-n4",
					PeerName: "2-4",
					Address:  "10.0.2.2",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/multipath_simple/node2",
					Destination: "/etc/node2",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"/usr/lib/frr/frrinit.sh start",
				"sleep 30",
				"gobgpd -f /etc/node2/config_zebra.toml",
			},
		},
		{
			Name: "node3",
			Interfaces: []topology.Interface{
				{
					Name:     "n3-n1",
					PeerName: "1-3",
					Address:  "10.0.1.3",
				},
				{
					Name:     "n3-n4",
					PeerName: "3-4",
					Address:  "10.0.3.2",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/multipath_simple/node3",
					Destination: "/etc/node3",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"/usr/lib/frr/frrinit.sh start",
				"sleep 30",
				"gobgpd -f /etc/node3/config_zebra.toml",
			},
		},
		{
			Name: "node4",
			Interfaces: []topology.Interface{
				{
					Name:     "n4-n2",
					PeerName: "2-4",
					Address:  "10.0.2.3",
				},
				{
					Name:     "n4-n3",
					PeerName: "3-4",
					Address:  "10.0.3.3",
				},
			},
			Image: "ghcr.io/terassyi/terakoya:0.1.2",
			Volume: []topology.Volume{
				{
					Source:      "./data/multipath_simple/node4",
					Destination: "/etc/node4",
				},
			},
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"/usr/lib/frr/frrinit.sh start",
				"sleep 30",
				"gobgpd -f /etc/node4/config_zebra.toml",
			},
		},
		{
			Name: "node5",
			Interfaces: []topology.Interface{
				{
					Name:     "n5-n1",
					PeerName: "1-5",
					Address:  "10.0.11.3",
				},
			},
			Image:        "ghcr.io/terassyi/terakoya:0.1.2",
			Privileged:   true,
			InitCommands: []string{"tail", "-f", "/dev/null"},
			Commands: []string{
				"ip route change default via 10.0.11.2",
			},
		},
	},
	Peers: []topology.Peer{
		{
			Name: "1-2",
			Cidr: "10.0.0.0/24",
		},
		{
			Name: "1-3",
			Cidr: "10.0.1.0/24",
		},
		{
			Name: "2-4",
			Cidr: "10.0.2.0/24",
		},
		{
			Name: "3-4",
			Cidr: "10.0.3.0/24",
		},
		{
			Name: "1-5",
			Cidr: "10.0.11.0/24",
		},
	},
}

var topologyName string

var rootCmd = &cobra.Command{
	Use:   "topology_builder",
	Short: "Topology builder for testing sart",
	Long:  "Topology builder for testing sart",
}

var buildCmd = &cobra.Command{
	Use:     "build",
	Short:   "Build topology",
	Long:    "Build topology",
	Example: "topology_builder build simple",
	RunE: func(cmd *cobra.Command, args []string) error {
		if len(args) == 0 || len(args) > 1 {
			return fmt.Errorf("Required one argument. Please run list command.")
		}
		name := args[0]
		topology, ok := TopologyMap[name]
		if !ok {
			return fmt.Errorf("Specified topology is not found. Please run list command.")
		}

		if err := topology.Build(context.Background()); err != nil {
			return err
		}
		return nil
	},
}

var cleanCmd = &cobra.Command{
	Use:     "clean",
	Short:   "Clean topology",
	Long:    "Clean topology",
	Example: "topology_builder clean simple",
	RunE: func(cmd *cobra.Command, args []string) error {
		if len(args) == 0 || len(args) > 1 {
			return fmt.Errorf("Required one argument. Please run list command.")
		}
		name := args[0]
		topology, ok := TopologyMap[name]
		if !ok {
			return fmt.Errorf("Specified topology is not found. Please run list command.")
		}

		if err := topology.Remove(context.Background()); err != nil {
			return err
		}
		return nil
	},
}

var listCmd = &cobra.Command{
	Use:     "list",
	Short:   "List available topologies",
	Long:    "List topologies",
	Example: "topology_builder list",
	Run: func(cmd *cobra.Command, args []string) {
		fmt.Println("Available topology list")
		names := []string{}
		for name := range TopologyMap {
			names = append(names, name)
		}
		sort.Strings(names)
		for _, name := range names {
			fmt.Printf("\t%s\n", name)
		}
	},
}

func init() {
	rootCmd.AddCommand(buildCmd)
	rootCmd.AddCommand(cleanCmd)
	rootCmd.AddCommand(listCmd)
}

func main() {
	rootCmd.Execute()
}
