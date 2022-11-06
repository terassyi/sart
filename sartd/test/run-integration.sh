#!/bin/bash
set -e

sartd/test/active_establish_peer.sh
sartd/test/passive_establish_peer.sh
sartd/test/reestablish_peer.sh
sartd/test/ipv6_establish_peer.sh
