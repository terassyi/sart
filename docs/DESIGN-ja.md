# Design of sart

Sart は自作のルーティングソフトウェア

## Concept

## sartd

Sart を構成するデーモンプログラム

### BGP

BGP の実装は [RFC 4271](https://www.rfc-editor.org/rfc/rfc4271)に基づく．
gRPC インターフェースを備えて外部より設定を注入できる．

#### 機能

目指す機能．
- [ ] eBGP
- [ ] iBGP
- [ ] 4 byte ASN
- [ ] IPv4 and IPv6 dual stack
- [ ] Route Reflector
- [ ] BGP unnumbered

##### スレッドモデル

### Netlink



## sart

Sart デーモンを制御する CLI ツール．

