#!/bin/bash
set -x
set -e

# args:
#   $1 = NS_NAME
function configure_zebra () {

	sudo mkdir -p /etc/frr/$1

	sudo cat <<__EOF__ > /etc/frr/$1/vtysh.conf
no service integrated-vtysh-config
hostname $1
__EOF__
	sudo sed -e 's/#watchfrr_options=""/watchfrr_options="--netns=NS_NAME"/' < /etc/frr/daemons > /etc/frr/$1/daemons
	sudo sed -i -e "s/NS_NAME/$1/" /etc/frr/$1/daemons
	sudo chown -R frr.frr "/etc/frr/$1"

}

configure_zebra node1
configure_zebra node2
configure_zebra node3
configure_zebra node4

/usr/lib/frr/frrinit.sh start "node2"
/usr/lib/frr/frrinit.sh start "node3"
/usr/lib/frr/frrinit.sh start "node4"
