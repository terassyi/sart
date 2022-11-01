#!/bin/bash
set -e
set -x

sartd/test/active_establish_peer.sh
sartd/test/passive_establish_peer.sh
sartd/test/reestablish_peer.sh
