#!/bin/bash

set -e

sudo ip netns del core
sudo ip netns del spine1
sudo ip netns del spine2
