#!/bin/bash
set -x

PIDS=$(ps aux | grep zebra | awk '( $1 == "root" || $1 == "frr") {print $2}')
for PID in $PIDS
do
	echo "sudo kill -9 ${PID}"
	sudo kill -9 $PID
done
