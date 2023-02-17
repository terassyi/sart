#!/bin/bash

ip netns exec core grpcurl -plaintext -d '{}' localhost:5000 sart.BgpApi.GetBgpInfo
