#!/bin/bash

ip netns exec core grpcurl -plaintext -d '{"family": {"afi": 1, "safi": 1}}' localhost:5000 sart.BgpApi.GetPath
