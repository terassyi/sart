# User Manual

This document describes how to use Sart.

Please refer to following links for details of each feature.

- [bgp](./bgp.md)
- [fib](./fib.md)
- [kubernetes](./kubernetes.md)

## Metrics

### Controller

|Name|Type|Description|
|---|---|---|
|sart_controller_reconciliation_total|Counter|Total count of reconciliations|
|sart_controller_reconciliation_errors_total|Counter|Total count of reconciliation errors|
|sart_controller_max_blocks|Gauge|The number of maximum allocatable address blocks|
|sart_controller_allocated_blocks|Gauge|The number of allocated address blocks|
|sart_controller_bgp_advertisements|Gauge|The number of BGP Advertisement|
|sart_controller_bgp_advertisement_status|Gauge|BGP Advertisement status|
|sart_controller_bgp_advertisement_backoff_count|Counter|The number of back off count of BGP Advertisement|

### Agent

|Name|Type|Description|
|---|---|---|
|sart_agent_reconciliation_total|Counter|Total count of reconciliations|
|sart_agent_reconciliation_errors_total|Counter|Total count of reconciliation errors|
|sart_agent_cni_call_total|Counter|Total count of CNI call|
|sart_agent_cni_call_errors_total|Counter|Total count of CNI call error|
|sart_agent_bgp_peer_status|Gauge|BGP peer status|
|sart_agent_node_bgp_status|Gauge|Node BGP status|
|sart_agent_node_bgp_backoff_count_total|Counter|NodeBGP backoff count|
