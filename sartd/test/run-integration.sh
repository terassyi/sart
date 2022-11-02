#!/bin/bash
set -e
set -x

echo "==== Test to establish BGP session"

sartd/test/simple/topology.sh
sartd/test/simple/gobgp_run.sh
sartd/test/simple/gobgp_stop.sh
