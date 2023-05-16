KIND_VERSION=0.18.0
KUBERNETES_VERSION=1.27.1
KUSTOMIZE_VERSION =5.0.1
GOBGP_VERSION := 3.10.0
CERT_MANAGER_VERSION := 1.11.2

BINDIR := $(abspath $(PWD)/../bin)

KIND := $(BINDIR)/kind
KUBECTL := $(BINDIR)/kubectl
KUSTOMIZE := $(BINDIR)/kustomize
export KUBECTL

KIND_CONFIG = kind-config.yaml

EXTERNAL_BGP_ASN ?= 65000
NODE0_ASN ?= 65000
NODE1_ASN ?= 65000
NODE2_ASN ?= 65000
NODE3_ASN ?= 65000
EXTERNAL_BGP_ADDR ?= ""
NODE0_ADDR ?= ""
NODE1_ADDR ?= ""
NODE2_ADDR ?= ""
NODE3_ADDR ?= ""
CLIENT_ADDR ?= ""
LB_CIDR ?= 10.69.0.0/24
ESCAPED_LB_CIDR ?= "10.69.0.0\/24"
ANOTHER_LB_CIDR ?= 10.69.10.0/24
ANOTHER_ESCAPED_LB_CIDR ?= "10.69.10.0\/24"

.PHONY: start
start: $(KIND) $(KUBECTL) $(KUSTOMIZE)
	$(KIND) create cluster --image kindest/node:v$(KUBERNETES_VERSION) --name sart --config $(KIND_CONFIG)

	$(KUBECTL) label nodes --overwrite sart-control-plane sart.terassyi.net/asn=${NODE0_ASN}
	$(KUBECTL) label nodes --overwrite sart-worker sart.terassyi.net/asn=${NODE1_ASN}
	$(KUBECTL) label nodes --overwrite sart-worker2 sart.terassyi.net/asn=${NODE2_ASN}
	$(KUBECTL) label nodes --overwrite sart-worker3 sart.terassyi.net/asn=${NODE3_ASN}

	$(eval NODE0_ADDR = $(shell $(KUBECTL) get nodes sart-control-plane -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}'))
	$(eval NODE1_ADDR = $(shell $(KUBECTL) get nodes sart-worker -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}'))
	$(eval NODE2_ADDR = $(shell $(KUBECTL) get nodes sart-worker2 -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}'))
	$(eval NODE3_ADDR = $(shell $(KUBECTL) get nodes sart-worker3 -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}'))

	docker run -d --privileged --network kind  --rm --ulimit core=-1 --name external-bgp --volume `pwd`/external-bgp/frr:/etc/frr/ ghcr.io/terassyi/terakoya:0.1.2 tail -f /dev/null
	docker run -d --privileged --network kind --rm --name external-client ghcr.io/terassyi/terakoya:0.1.2 tail -f /dev/null

	make configure

.PHONY: configure
configure:
	docker exec external-bgp /usr/lib/frr/frrinit.sh start

	$(eval NODE0_ADDR = $(shell $(KUBECTL) get nodes sart-control-plane -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}'))
	$(eval NODE1_ADDR = $(shell $(KUBECTL) get nodes sart-worker -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}'))
	$(eval NODE2_ADDR = $(shell $(KUBECTL) get nodes sart-worker2 -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}'))
	$(eval NODE3_ADDR = $(shell $(KUBECTL) get nodes sart-worker3 -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}'))
	$(eval EXTERNAL_BGP_ADDR = $(shell docker inspect external-bgp | jq '.[0].NetworkSettings.Networks.kind.IPAddress' | tr -d '"'))

	sed -e s/NODE0_ASN/${NODE0_ASN}/g -e s/NODE1_ASN/${NODE1_ASN}/g -e s/NODE2_ASN/${NODE2_ASN}/g -e s/NODE3_ASN/${NODE3_ASN}/g \
		-e s/EXTERNAL_BGP_ASN/${EXTERNAL_BGP_ASN}/g \
		-e s/EXTERNAL_BGP_ADDR/${EXTERNAL_BGP_ADDR}/g \
		-e s/NODE0_ADDR/${NODE0_ADDR}/g -e s/NODE1_ADDR/${NODE1_ADDR}/g -e s/NODE2_ADDR/${NODE2_ADDR}/g -e s/NODE3_ADDR/${NODE3_ADDR}/g \
		./external-bgp/frr/gobgp.conf.tmpl > ./external-bgp/frr/gobgp.conf

	sed -e s/LB_CIDR/'${ESCAPED_LB_CIDR}'/g \
		./manifests/templates/addresspool.yaml.tmpl > ./manifests/addresspool.yaml

	sed -e s/ANOTHER_LB_CIDR/'${ANOTHER_ESCAPED_LB_CIDR}'/g \
		./manifests/templates/another_addresspool.yaml.tmpl > ./manifests/another_addresspool.yaml

	sed -e s/EXTERNAL_BGP_ASN/${EXTERNAL_BGP_ASN}/g -e s/EXTERNAL_BGP_ADDR/${EXTERNAL_BGP_ADDR}/g \
		./manifests/templates/peer.yaml.tmpl > ./manifests/peer.yaml

	docker exec -d external-bgp gobgpd -f /etc/frr/gobgp.conf
	docker exec external-client ip route add ${LB_CIDR} via ${EXTERNAL_BGP_ADDR}
	docker exec external-client ip route add ${ANOTHER_LB_CIDR} via ${EXTERNAL_BGP_ADDR}


.PHONY: install
install:
	$(KIND) load docker-image --name sart sart:dev
	$(KIND) load docker-image --name sart sart-controller:dev
	$(KIND) load docker-image --name sart test-app:dev
	$(KUBECTL) apply -f https://github.com/jetstack/cert-manager/releases/download/v$(CERT_MANAGER_VERSION)/cert-manager.yaml
	$(KUBECTL) -n cert-manager wait --for=condition=available --timeout=180s --all deployments
	$(KUSTOMIZE) build --load-restrictor=LoadRestrictionsNone . | $(KUBECTL) apply -f -
	$(KUBECTL) -n kube-system wait --timeout=3m --for=condition=available deployment/sart-controller

.PHONY: stop
stop: $(KIND)
	$(KIND) delete cluster --name sart
	docker rm -f external-bgp
	docker rm -f external-client

.PHONY: test
test:
	go test -count 1 -v . -ginkgo.v

.PHONY: logs
logs:
	rm -rf logs.tar.gz logs
	$(KIND) export logs --name=coil ./logs
	tar czf logs.tar.gz logs
	rm -rf logs

$(KIND):
	mkdir -p $(dir $@)
	curl -sfL -o $@ https://github.com/kubernetes-sigs/kind/releases/download/v$(KIND_VERSION)/kind-linux-amd64
	chmod a+x $@

$(KUBECTL):
	mkdir -p $(dir $@)
	curl -sfL -o $@ https://dl.k8s.io/release/v$(KUBERNETES_VERSION)/bin/linux/amd64/kubectl
	chmod a+x $@

$(KUSTOMIZE):
	mkdir -p $(dir $@)
	curl -sfL https://github.com/kubernetes-sigs/kustomize/releases/download/kustomize%2Fv$(KUSTOMIZE_VERSION)/kustomize_v$(KUSTOMIZE_VERSION)_linux_amd64.tar.gz | tar -xz -C $(BINDIR)
	chmod a+x $@
