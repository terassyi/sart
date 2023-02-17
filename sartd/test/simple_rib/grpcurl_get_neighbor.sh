#!/bin/bash

ip netns exec core grpcurl -plaintext -d '{"addr": "10.0.0.2"}' localhost:5000 sart.BgpApi.GetNeighbor
