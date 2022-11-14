#!/bin/bash
set -e

PIDS=$(ps aux | grep gobgp | awk '( $1 == "root") {print $2}')
for PID in $PIDS
do
	echo "sudo kill -9 ${PID}"
	sudo kill -9 $PID
done
